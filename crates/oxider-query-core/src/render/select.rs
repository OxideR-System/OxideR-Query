//! SELECT rendering: CTEs, sources, joins, grouping, set operations, ordering,
//! pagination, and locking.

use crate::ast::node::{Node, WhenArm};
use crate::ast::operator::Operator;
use crate::ast::query::{
    Cte, Distinct, JoinAst, JoinKind, LockMode, LockWait, NullsOrder, OrderAst, SelectAst, SetOp,
    Source,
};
use crate::dialect::Dialect;
use crate::render::{RenderResult, Rendered, Renderer};

/// Render a SELECT for a dialect, returning its SQL and bound parameters.
pub fn render_select_into(query: &SelectAst, dialect: &dyn Dialect) -> RenderResult<Rendered> {
    let mut renderer = Renderer::new(dialect);
    renderer.select(query)?;
    Ok(renderer.finish())
}

impl Renderer<'_> {
    /// Render a complete SELECT statement.
    pub(crate) fn select(&mut self, query: &SelectAst) -> RenderResult {
        if !query.with.is_empty() {
            self.with_clause(query)?;
        }
        self.select_body(query)?;
        self.set_operations(query)?;
        self.tail(query)?;
        Ok(())
    }

    /// Render the `WITH` clause.
    fn with_clause(&mut self, query: &SelectAst) -> RenderResult {
        if !self.dialect().caps().cte {
            return self.unsupported("common table expressions");
        }
        if query.recursive && !self.dialect().caps().recursive_cte {
            return self.unsupported("recursive common table expressions");
        }
        self.push(if query.recursive {
            "WITH RECURSIVE "
        } else {
            "WITH "
        });
        self.comma_separated(&query.with, Self::cte)?;
        self.push(" ");
        Ok(())
    }

    /// Render one common table expression.
    fn cte(&mut self, cte: &Cte) -> RenderResult {
        self.ident(cte.name);
        if !cte.columns.is_empty() {
            self.push(" (");
            self.comma_separated(&cte.columns, |r, name| {
                r.ident(name);
                Ok(())
            })?;
            self.push(")");
        }
        self.push(" AS (");
        self.select(&cte.query)?;
        self.push(")");
        Ok(())
    }

    /// Render everything from `SELECT` through `HAVING` and the `WINDOW`
    /// clause, but not the trailing ORDER BY / LIMIT / locking, which belong to
    /// the whole set operation when there is one.
    fn select_body(&mut self, query: &SelectAst) -> RenderResult {
        self.push("SELECT ");
        match &query.distinct {
            Distinct::No => {}
            Distinct::All => self.push("DISTINCT "),
            Distinct::On(exprs) => {
                if !self.dialect().caps().distinct_on {
                    return self.unsupported("DISTINCT ON");
                }
                self.push("DISTINCT ON (");
                self.comma_separated(exprs, |r, node| r.expr(node))?;
                self.push(") ");
            }
        }

        if query.columns.is_empty() {
            self.push("*");
        } else {
            self.comma_separated(&query.columns, |r, node| r.expr(node))?;
        }

        if query.from.is_empty() {
            if let Some(dummy) = self.dialect().dummy_from() {
                self.push(" FROM ");
                self.push(dummy);
            }
        } else {
            self.push(" FROM ");
            self.comma_separated(&query.from, Self::source)?;
        }

        for join in &query.joins {
            self.join(join)?;
        }

        if let Some(filter) = &query.filter {
            self.push(" WHERE ");
            self.expr(filter)?;
        }

        if !query.group.is_empty() {
            self.push(" GROUP BY ");
            self.comma_separated(&query.group, |r, node| r.expr(node))?;
        }

        if let Some(having) = &query.having {
            self.push(" HAVING ");
            self.expr(having)?;
        }

        if !query.windows.is_empty() {
            if !self.dialect().caps().named_windows {
                return self.unsupported("named windows");
            }
            self.push(" WINDOW ");
            self.comma_separated(&query.windows, |r, (name, window)| {
                r.ident(name);
                r.push(" AS (");
                r.inline_window(window)?;
                r.push(")");
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Render a FROM or JOIN source.
    fn source(&mut self, source: &Source) -> RenderResult {
        match source {
            Source::Table(table) => {
                if let Some(schema) = table.schema {
                    self.ident(schema);
                    self.push(".");
                }
                self.ident(table.name);
                if let Some(alias) = table.alias {
                    self.push(" AS ");
                    self.ident(alias);
                }
                Ok(())
            }
            Source::Derived { query, alias } => {
                self.push("(");
                self.select(query)?;
                self.push(") AS ");
                self.ident(alias);
                Ok(())
            }
        }
    }

    /// Render one JOIN clause.
    fn join(&mut self, join: &JoinAst) -> RenderResult {
        let caps = self.dialect().caps();
        match join.kind {
            JoinKind::Right if !caps.right_join => return self.unsupported("RIGHT JOIN"),
            JoinKind::Full if !caps.full_join => return self.unsupported("FULL JOIN"),
            _ => {}
        }
        self.push(" ");
        self.push(join.kind.as_sql());
        self.push(" ");
        self.source(&join.source)?;
        if let Some(on) = &join.on {
            self.push(" ON ");
            self.expr(on)?;
        }
        Ok(())
    }

    /// Render any set operations chained onto this query.
    fn set_operations(&mut self, query: &SelectAst) -> RenderResult {
        for (op, branch) in &query.set_ops {
            let caps = self.dialect().caps();
            match op {
                SetOp::Intersect | SetOp::IntersectAll if !caps.intersect => {
                    return self.unsupported("INTERSECT")
                }
                SetOp::Except | SetOp::ExceptAll if !caps.except => {
                    return self.unsupported("EXCEPT")
                }
                SetOp::IntersectAll | SetOp::ExceptAll if !caps.set_op_all => {
                    return self.unsupported("the ALL form of INTERSECT and EXCEPT")
                }
                _ => {}
            }
            self.push(" ");
            self.push(op.as_sql());
            self.push(" ");
            let wrap = caps.wrap_set_op_branches;
            if wrap {
                self.push("(");
            }
            // A branch's own ORDER BY and LIMIT belong to the branch; the outer
            // query's belong to the whole set operation and are rendered after.
            self.select(branch)?;
            if wrap {
                self.push(")");
            }
        }
        Ok(())
    }

    /// Render ORDER BY, LIMIT, OFFSET, and the locking clause.
    fn tail(&mut self, query: &SelectAst) -> RenderResult {
        if !query.order.is_empty() {
            self.push(" ORDER BY ");
            self.order_terms(&query.order)?;
        }
        self.pagination(query.limit, query.offset)?;
        if let Some(lock) = query.lock {
            if !self.dialect().caps().row_locking {
                return self.unsupported("row locking clauses");
            }
            self.push(match lock.mode {
                LockMode::Update => " FOR UPDATE",
                LockMode::Share => " FOR SHARE",
                LockMode::NoKeyUpdate => " FOR NO KEY UPDATE",
                LockMode::KeyShare => " FOR KEY SHARE",
            });
            match lock.wait {
                LockWait::Wait => {}
                LockWait::NoWait | LockWait::SkipLocked => {
                    if !self.dialect().caps().lock_wait_policy {
                        return self.unsupported("NOWAIT and SKIP LOCKED");
                    }
                    self.push(if lock.wait == LockWait::NoWait {
                        " NOWAIT"
                    } else {
                        " SKIP LOCKED"
                    });
                }
            }
        }
        Ok(())
    }

    /// Render LIMIT and OFFSET, supplying the placeholder limit that engines
    /// requiring `LIMIT` before `OFFSET` need.
    ///
    /// Both are non-negative integers the caller supplied as numbers, never as
    /// text, so inlining them cannot inject anything.
    fn pagination(&mut self, limit: Option<u64>, offset: Option<u64>) -> RenderResult {
        match (limit, offset) {
            (Some(n), _) => {
                self.push(" LIMIT ");
                self.push(&n.to_string());
            }
            (None, Some(_)) => {
                if let Some(unlimited) = self.dialect().unlimited_limit() {
                    self.push(" LIMIT ");
                    self.push(unlimited);
                }
            }
            (None, None) => {}
        }
        if let Some(m) = offset {
            self.push(" OFFSET ");
            self.push(&m.to_string());
        }
        Ok(())
    }

    /// Render a list of ORDER BY terms, emulating null placement where the
    /// dialect has no native `NULLS FIRST` / `NULLS LAST`.
    pub(crate) fn order_terms(&mut self, terms: &[OrderAst]) -> RenderResult {
        let native_nulls = self.dialect().caps().nulls_ordering;
        let mut first = true;
        for term in terms {
            if !first {
                self.push(", ");
            }
            first = false;

            if !native_nulls && term.nulls != NullsOrder::Default {
                // A leading sort key that is 0 or 1 depending on nullness puts
                // the nulls on the requested side, which is how QueryDSL
                // emulates this on MySQL and SQL Server too.
                let (when_null, when_present) = match term.nulls {
                    NullsOrder::First => ("0", "1"),
                    _ => ("1", "0"),
                };
                self.expr(&null_rank(&term.expr, when_null, when_present))?;
                self.push(", ");
            }

            self.expr(&term.expr)?;
            self.push(" ");
            self.push(term.dir.as_sql());

            if native_nulls {
                match term.nulls {
                    NullsOrder::Default => {}
                    NullsOrder::First => self.push(" NULLS FIRST"),
                    NullsOrder::Last => self.push(" NULLS LAST"),
                }
            }
        }
        Ok(())
    }
}

/// `CASE WHEN <expr> IS NULL THEN <when_null> ELSE <when_present> END`, the
/// sort key that emulates null ordering.
fn null_rank(expr: &Node, when_null: &'static str, when_present: &'static str) -> Node {
    Node::Case {
        operand: None,
        arms: vec![WhenArm {
            when: Node::unary(Operator::IsNull, expr.clone()),
            then: Node::Keyword(when_null),
        }],
        otherwise: Some(Box::new(Node::Keyword(when_present))),
    }
}

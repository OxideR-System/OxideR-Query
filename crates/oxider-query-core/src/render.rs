//! Renders a dialect-agnostic query AST into SQL text plus ordered bound params.

use crate::dialect::Dialect;
use crate::expr::Expr;
use crate::predicate::OrderDir;
use crate::query::SelectQuery;
use crate::value::Value;
use core::fmt::Write;

/// Rendered SQL alongside its ordered bound parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// The SQL string with dialect-specific placeholders.
    pub sql: String,
    /// Bound parameters in placeholder order.
    pub params: Vec<Value>,
}

impl SelectQuery {
    /// Render this query for a specific dialect.
    pub fn render<D: Dialect>(&self, dialect: &D) -> Rendered {
        let mut params = Vec::new();
        let sql = render_select(self, dialect, &mut params);
        Rendered { sql, params }
    }
}

/// Render a SELECT into SQL text, appending its bound values to `params`. Shared
/// by the top-level [`SelectQuery::render`] and by subquery expressions so that
/// placeholders stay in a single global order.
pub(crate) fn render_select<D: Dialect>(
    query: &SelectQuery,
    dialect: &D,
    params: &mut Vec<Value>,
) -> String {
    let SelectQuery {
        from,
        joins,
        columns,
        filter,
        group,
        having,
        order,
        limit,
        offset,
    } = query;
    let mut sql = String::from("SELECT ");

    if columns.is_empty() {
        sql.push('*');
    } else {
        let cols: Vec<String> = columns
            .iter()
            .map(|c| render_expr(c, dialect, params))
            .collect();
        sql.push_str(&cols.join(", "));
    }

    sql.push_str(" FROM ");
    sql.push_str(&dialect.quote_ident(from));

    for join in joins {
        sql.push(' ');
        sql.push_str(join.kind.as_sql());
        sql.push(' ');
        sql.push_str(&dialect.quote_ident(join.table));
        sql.push_str(" ON ");
        sql.push_str(&render_expr(&join.on, dialect, params));
    }

    if let Some(filter) = filter {
        sql.push_str(" WHERE ");
        sql.push_str(&render_expr(filter, dialect, params));
    }

    if !group.is_empty() {
        sql.push_str(" GROUP BY ");
        let cols: Vec<String> = group
            .iter()
            .map(|c| render_expr(c, dialect, params))
            .collect();
        sql.push_str(&cols.join(", "));
    }

    if let Some(having) = having {
        sql.push_str(" HAVING ");
        sql.push_str(&render_expr(having, dialect, params));
    }

    if !order.is_empty() {
        sql.push_str(" ORDER BY ");
        let terms: Vec<String> = order
            .iter()
            .map(|t| {
                let expr = render_expr(&t.expr, dialect, params);
                let dir = match t.dir {
                    OrderDir::Asc => "ASC",
                    OrderDir::Desc => "DESC",
                };
                format!("{expr} {dir}")
            })
            .collect();
        sql.push_str(&terms.join(", "));
    }

    // LIMIT/OFFSET take non-negative integer literals, so inlining them is
    // safe (no user-controlled string) and uniform across the target dialects.
    if let Some(n) = limit {
        let _ = write!(sql, " LIMIT {n}");
    }
    if let Some(m) = offset {
        let _ = write!(sql, " OFFSET {m}");
    }

    sql
}

/// Recursively render an expression, appending bound values to `params`.
pub(crate) fn render_expr<D: Dialect>(expr: &Expr, dialect: &D, params: &mut Vec<Value>) -> String {
    match expr {
        Expr::Column { table, name } => {
            format!(
                "{}.{}",
                dialect.quote_ident(table),
                dialect.quote_ident(name)
            )
        }
        Expr::Param(value) => {
            params.push(value.clone());
            dialect.placeholder(params.len())
        }
        Expr::Binary { op, lhs, rhs } => {
            let l = render_expr(lhs, dialect, params);
            let r = render_expr(rhs, dialect, params);
            format!("({} {} {})", l, op.as_sql(), r)
        }
        Expr::Aggregate { func, arg } => {
            let inner = match arg {
                Some(expr) => render_expr(expr, dialect, params),
                None => "*".to_string(),
            };
            format!("{}({})", func.as_sql(), inner)
        }
        Expr::Subquery(query) => {
            format!("({})", render_select(query, dialect, params))
        }
        Expr::Exists { negated, subquery } => {
            let kw = if *negated { "NOT EXISTS" } else { "EXISTS" };
            format!("{} ({})", kw, render_select(subquery, dialect, params))
        }
    }
}

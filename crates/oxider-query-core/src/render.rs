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
        let mut sql = String::from("SELECT ");

        if self.columns.is_empty() {
            sql.push('*');
        } else {
            let cols: Vec<String> = self
                .columns
                .iter()
                .map(|c| render_expr(c, dialect, &mut params))
                .collect();
            sql.push_str(&cols.join(", "));
        }

        sql.push_str(" FROM ");
        sql.push_str(&dialect.quote_ident(self.from));

        for join in &self.joins {
            sql.push(' ');
            sql.push_str(join.kind.as_sql());
            sql.push(' ');
            sql.push_str(&dialect.quote_ident(join.table));
            sql.push_str(" ON ");
            sql.push_str(&render_expr(&join.on, dialect, &mut params));
        }

        if let Some(filter) = &self.filter {
            sql.push_str(" WHERE ");
            sql.push_str(&render_expr(filter, dialect, &mut params));
        }

        if !self.order.is_empty() {
            sql.push_str(" ORDER BY ");
            let terms: Vec<String> = self
                .order
                .iter()
                .map(|t| {
                    let expr = render_expr(&t.expr, dialect, &mut params);
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
        if let Some(n) = self.limit {
            let _ = write!(sql, " LIMIT {n}");
        }
        if let Some(m) = self.offset {
            let _ = write!(sql, " OFFSET {m}");
        }

        Rendered { sql, params }
    }
}

/// Recursively render an expression, appending bound values to `params`.
fn render_expr<D: Dialect>(expr: &Expr, dialect: &D, params: &mut Vec<Value>) -> String {
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
    }
}

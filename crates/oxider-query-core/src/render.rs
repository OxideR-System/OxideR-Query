//! Renders a dialect-agnostic query AST into SQL text plus ordered bound params.

use crate::dialect::Dialect;
use crate::expr::Expr;
use crate::query::SelectQuery;
use crate::value::Value;

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

        if let Some(from) = self.from {
            sql.push_str(" FROM ");
            sql.push_str(&dialect.quote_ident(from));
        }

        if let Some(filter) = &self.filter {
            sql.push_str(" WHERE ");
            sql.push_str(&render_expr(filter, dialect, &mut params));
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

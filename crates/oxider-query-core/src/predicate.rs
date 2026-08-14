//! Predicates (boolean expressions for WHERE) and ORDER BY terms.

use crate::expr::{BinOp, Expr};

/// A boolean predicate usable in a WHERE clause.
///
/// Produced by column comparison methods; combine with [`Predicate::and`] and
/// [`Predicate::or`].
pub struct Predicate(Expr);

impl Predicate {
    /// Wrap a raw boolean expression. Internal to column construction.
    pub(crate) fn new(expr: Expr) -> Self {
        Predicate(expr)
    }

    /// Consume into the underlying AST. Internal to the builder.
    pub(crate) fn into_expr(self) -> Expr {
        self.0
    }

    /// Combine with another predicate using `AND`.
    pub fn and(self, other: Predicate) -> Predicate {
        Predicate(Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(self.0),
            rhs: Box::new(other.0),
        })
    }

    /// Combine with another predicate using `OR`.
    pub fn or(self, other: Predicate) -> Predicate {
        Predicate(Expr::Binary {
            op: BinOp::Or,
            lhs: Box::new(self.0),
            rhs: Box::new(other.0),
        })
    }
}

/// Sort direction for an ORDER BY term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    /// Ascending (`ASC`).
    Asc,
    /// Descending (`DESC`).
    Desc,
}

/// A single ORDER BY term: an expression plus a direction.
pub struct OrderTerm {
    pub(crate) expr: Expr,
    pub(crate) dir: OrderDir,
}

impl OrderTerm {
    /// Build an order term. Internal to column construction.
    pub(crate) fn new(expr: Expr, dir: OrderDir) -> Self {
        OrderTerm { expr, dir }
    }
}

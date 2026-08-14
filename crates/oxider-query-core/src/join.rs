//! JOIN clauses.
//!
//! Phase 3a renders joins and lets a query reference columns from any joined
//! entity. Type-level enforcement that a referenced column belongs to a joined
//! table (and outer-join nullability) is a later phase; for now the builder
//! trusts the caller, exactly like a hand-written query.

use crate::expr::Expr;

/// The kind of join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// `INNER JOIN`
    Inner,
    /// `LEFT JOIN` (left outer join).
    Left,
}

impl JoinKind {
    /// The SQL keyword for this join kind.
    pub fn as_sql(self) -> &'static str {
        match self {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
        }
    }
}

/// A single JOIN clause: kind, target table, and ON condition.
pub struct Join {
    pub(crate) kind: JoinKind,
    pub(crate) table: &'static str,
    pub(crate) on: Expr,
}

impl Join {
    /// Build a join clause. Internal to the query builder.
    pub(crate) fn new(kind: JoinKind, table: &'static str, on: Expr) -> Self {
        Join { kind, table, on }
    }
}

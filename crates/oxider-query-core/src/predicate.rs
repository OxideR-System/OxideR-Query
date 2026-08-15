//! Predicates and ORDER BY terms, parameterized by the entities they reference.
//!
//! `Predicate<S>` and `Order<S>` carry a type-level source set `S` (see
//! [`crate::source`]) recording which entities the expression touches. The query
//! builder checks that set against the entities in scope. The runtime AST it
//! stores ([`OrderTerm`], and the `Expr` inside a predicate) is source-erased.

use crate::expr::{BinOp, Expr};
use crate::source::Concat;
use core::marker::PhantomData;

/// A boolean predicate for a WHERE clause, referencing the entities in `S`.
///
/// Produced by column comparison methods; combine with [`Predicate::and`] and
/// [`Predicate::or`], which merge the referenced source sets.
pub struct Predicate<S> {
    expr: Expr,
    _sources: PhantomData<fn() -> S>,
}

impl<S> Predicate<S> {
    /// Wrap a raw boolean expression. Internal to column construction.
    pub(crate) fn new(expr: Expr) -> Self {
        Predicate {
            expr,
            _sources: PhantomData,
        }
    }

    /// Consume into the underlying AST. Internal to the builder.
    pub(crate) fn into_expr(self) -> Expr {
        self.expr
    }

    /// Combine with another predicate using `AND`; the result references the
    /// union of both operands' entities.
    pub fn and<S2>(self, other: Predicate<S2>) -> Predicate<<S as Concat<S2>>::Out>
    where
        S: Concat<S2>,
    {
        Predicate::new(Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(self.expr),
            rhs: Box::new(other.expr),
        })
    }

    /// Combine with another predicate using `OR`.
    pub fn or<S2>(self, other: Predicate<S2>) -> Predicate<<S as Concat<S2>>::Out>
    where
        S: Concat<S2>,
    {
        Predicate::new(Expr::Binary {
            op: BinOp::Or,
            lhs: Box::new(self.expr),
            rhs: Box::new(other.expr),
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

/// A source-erased ORDER BY term in the query AST.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderTerm {
    pub(crate) expr: Expr,
    pub(crate) dir: OrderDir,
}

impl OrderTerm {
    pub(crate) fn new(expr: Expr, dir: OrderDir) -> Self {
        OrderTerm { expr, dir }
    }
}

/// A typed ORDER BY term referencing the entities in `S`, returned by a column's
/// `asc`/`desc`. The builder checks `S` before storing the erased [`OrderTerm`].
pub struct Order<S> {
    term: OrderTerm,
    _sources: PhantomData<fn() -> S>,
}

impl<S> Order<S> {
    /// Build a typed order term. Internal to column construction.
    pub(crate) fn new(expr: Expr, dir: OrderDir) -> Self {
        Order {
            term: OrderTerm::new(expr, dir),
            _sources: PhantomData,
        }
    }

    /// Consume into the erased AST term. Internal to the builder.
    pub(crate) fn into_term(self) -> OrderTerm {
        self.term
    }
}

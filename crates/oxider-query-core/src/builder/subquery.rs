//! Subqueries used as expressions.
//!
//! A [`Subquery`] is a finished SELECT viewed as a value. Its source set is the
//! *free* entities of the inner query, never the tables it selects from, so an
//! uncorrelated subquery drops into any query and a correlated one only fits
//! where its outer table is in scope.

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::ast::query::SelectAst;
use crate::builder::select::Select;
use crate::source::Concat;
use crate::typed::expr::{Expr, IntoExpr, Predicate};
use crate::typed::ops_compare::Merge;
use crate::typed::selection::AnyExpr;
use core::marker::PhantomData;

/// A SELECT usable as an expression of type `T`.
pub struct Subquery<S, T> {
    ast: SelectAst,
    _marker: PhantomData<fn() -> (S, T)>,
}

impl<S, T> Subquery<S, T> {
    /// Wrap a finished statement.
    pub(crate) fn new(ast: SelectAst) -> Self {
        Subquery {
            ast,
            _marker: PhantomData,
        }
    }

    /// The wrapped statement.
    pub fn into_ast(self) -> SelectAst {
        self.ast
    }

    /// Lower into an AST node.
    pub fn into_node(self) -> Node {
        Node::Subquery(Box::new(self.ast))
    }

    /// `ANY (subquery)` - satisfied when the comparison holds for at least one
    /// returned row.
    ///
    /// Used as an operand: `Item::price.gt(cheap.any())`.
    pub fn any(self) -> Expr<S, T> {
        Expr::new(Node::unary(Operator::Any, self.into_node()))
    }

    /// `ALL (subquery)` - satisfied when the comparison holds for every
    /// returned row, vacuously true when it returns none.
    pub fn all(self) -> Expr<S, T> {
        Expr::new(Node::unary(Operator::All, self.into_node()))
    }
}

impl<S, T> IntoExpr<T> for Subquery<S, T> {
    type Sources = S;
    fn into_expr_node(self) -> Node {
        self.into_node()
    }
}

impl<S, T> AnyExpr for Subquery<S, T> {
    type Sources = S;
    type Output = T;
    fn into_any_node(self) -> Node {
        self.into_node()
    }
}

impl<S, T> Clone for Subquery<S, T> {
    fn clone(&self) -> Self {
        Subquery::new(self.ast.clone())
    }
}

impl<S, T> core::fmt::Debug for Subquery<S, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Subquery").field(&self.ast).finish()
    }
}

/// `EXISTS (subquery)` - true when the query returns at least one row.
///
/// Takes the query rather than a [`Subquery`] because `EXISTS` ignores the
/// projection entirely.
pub fn exists<S, F>(query: Select<S, F>) -> Predicate<F> {
    Expr::new(Node::unary(
        Operator::Exists,
        Node::Subquery(Box::new(query.into_ast())),
    ))
}

/// `NOT EXISTS (subquery)` - true when the query returns no rows.
pub fn not_exists<S, F>(query: Select<S, F>) -> Predicate<F> {
    Expr::new(Node::unary(
        Operator::NotExists,
        Node::Subquery(Box::new(query.into_ast())),
    ))
}

/// Membership tests against a subquery.
///
/// Kept separate from [`CompareOps`](crate::typed::CompareOps) so the operator
/// traits stay independent of the builder, which depends on them.
pub trait SubqueryOps<T>: IntoExpr<T> + Sized {
    /// `self IN (subquery)`
    fn in_subquery<S2>(self, subquery: Subquery<S2, T>) -> Predicate<Merge<Self::Sources, S2>>
    where
        Self::Sources: Concat<S2>,
    {
        Expr::new(Node::binary(
            Operator::In,
            self.into_expr_node(),
            subquery.into_node(),
        ))
    }

    /// `self NOT IN (subquery)`
    fn not_in_subquery<S2>(self, subquery: Subquery<S2, T>) -> Predicate<Merge<Self::Sources, S2>>
    where
        Self::Sources: Concat<S2>,
    {
        Expr::new(Node::binary(
            Operator::NotIn,
            self.into_expr_node(),
            subquery.into_node(),
        ))
    }
}

impl<E, T> SubqueryOps<T> for crate::typed::Column<E, T> {}
impl<S, T> SubqueryOps<T> for Expr<S, T> {}

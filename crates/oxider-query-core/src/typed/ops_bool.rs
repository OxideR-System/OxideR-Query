//! Boolean connectives.
//!
//! `and` and `or` are also available as the `&` and `|` operators, so a
//! predicate can be written either way:
//!
//! ```ignore
//! User::age.ge(18).and(User::active.eq(true))
//! User::age.ge(18) & User::active.eq(true)
//! ```
//!
//! Negation is only the `not` method. `std::ops::Not` would give `!pred` too,
//! but then `pred.not()` becomes ambiguous between the two traits, and a
//! readable method matters more here than the operator.

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::source::Concat;
use crate::typed::expr::{Expr, IntoExpr, Predicate};
use crate::typed::ops_compare::Merge;
use core::ops::{BitAnd, BitOr};

/// Boolean combination, available on any expression of type `bool`.
pub trait BoolOps: IntoExpr<bool> + Sized {
    /// `self AND rhs`
    fn and<R>(self, rhs: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<bool>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::And,
            self.into_expr_node(),
            rhs.into_expr_node(),
        ))
    }

    /// `self OR rhs`
    fn or<R>(self, rhs: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<bool>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::Or,
            self.into_expr_node(),
            rhs.into_expr_node(),
        ))
    }

    /// `self XOR rhs`
    fn xor<R>(self, rhs: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<bool>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::Xor,
            self.into_expr_node(),
            rhs.into_expr_node(),
        ))
    }

    /// `NOT self`
    fn not(self) -> Predicate<Self::Sources> {
        Expr::new(Node::unary(Operator::Not, self.into_expr_node()))
    }

    /// Combine with `AND` only when the operand is present.
    ///
    /// The building block for dynamic filters: `p.and_opt(maybe)` keeps `p`
    /// unchanged when `maybe` is `None`, so a chain of optional conditions
    /// needs no `if let` at each step. The operand must reference the same
    /// entities as `self`, since its absence cannot change the type.
    fn and_opt(self, rhs: Option<Predicate<Self::Sources>>) -> Predicate<Self::Sources> {
        match rhs {
            Some(rhs) => Expr::new(Node::binary(
                Operator::And,
                self.into_expr_node(),
                rhs.into_node(),
            )),
            None => Expr::new(self.into_expr_node()),
        }
    }

    /// Combine with `OR` only when the operand is present.
    fn or_opt(self, rhs: Option<Predicate<Self::Sources>>) -> Predicate<Self::Sources> {
        match rhs {
            Some(rhs) => Expr::new(Node::binary(
                Operator::Or,
                self.into_expr_node(),
                rhs.into_node(),
            )),
            None => Expr::new(self.into_expr_node()),
        }
    }
}

impl<E> BoolOps for crate::typed::Column<E, bool> {}
impl<S> BoolOps for Expr<S, bool> {}

impl<S, R> BitAnd<R> for Expr<S, bool>
where
    R: IntoExpr<bool>,
    S: Concat<R::Sources>,
{
    type Output = Predicate<Merge<S, R::Sources>>;

    fn bitand(self, rhs: R) -> Self::Output {
        Expr::new(Node::binary(
            Operator::And,
            self.into_node(),
            rhs.into_expr_node(),
        ))
    }
}

impl<S, R> BitOr<R> for Expr<S, bool>
where
    R: IntoExpr<bool>,
    S: Concat<R::Sources>,
{
    type Output = Predicate<Merge<S, R::Sources>>;

    fn bitor(self, rhs: R) -> Self::Output {
        Expr::new(Node::binary(
            Operator::Or,
            self.into_node(),
            rhs.into_expr_node(),
        ))
    }
}

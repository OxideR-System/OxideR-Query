//! `CASE` expressions.
//!
//! QueryDSL's `CaseBuilder` alternates two states (`when(...)` then `then(...)`)
//! and needs a separate class for each. Taking the condition and its result
//! together collapses that to one type, which keeps the generic bookkeeping
//! readable while building exactly the same AST:
//!
//! ```ignore
//! case_when(User::age.lt(18), "minor")
//!     .when(User::age.lt(65), "adult")
//!     .otherwise("senior")
//! ```

use crate::ast::node::{Node, WhenArm};
use crate::source::Concat;
use crate::typed::expr::{Expr, IntoExpr};
use crate::typed::ops_compare::{Merge, Merge3};
use core::marker::PhantomData;

/// A `CASE` expression under construction, with at least one arm.
///
/// `T` is the type every arm produces, so mixing a text arm and a numeric arm
/// is a compile error.
pub struct CaseBuilder<S, T> {
    arms: Vec<WhenArm>,
    _marker: PhantomData<fn() -> (S, T)>,
}

/// Start a `CASE` with its first `WHEN ... THEN ...` arm.
pub fn case_when<P, V, T>(condition: P, result: V) -> CaseBuilder<Merge<P::Sources, V::Sources>, T>
where
    P: IntoExpr<bool>,
    V: IntoExpr<T>,
    P::Sources: Concat<V::Sources>,
{
    CaseBuilder {
        arms: vec![WhenArm {
            when: condition.into_expr_node(),
            then: result.into_expr_node(),
        }],
        _marker: PhantomData,
    }
}

impl<S, T> CaseBuilder<S, T> {
    /// Add another `WHEN ... THEN ...` arm. Arms are tried in order.
    pub fn when<P, V>(
        mut self,
        condition: P,
        result: V,
    ) -> CaseBuilder<Merge3<S, P::Sources, V::Sources>, T>
    where
        P: IntoExpr<bool>,
        V: IntoExpr<T>,
        P::Sources: Concat<V::Sources>,
        S: Concat<Merge<P::Sources, V::Sources>>,
    {
        self.arms.push(WhenArm {
            when: condition.into_expr_node(),
            then: result.into_expr_node(),
        });
        CaseBuilder {
            arms: self.arms,
            _marker: PhantomData,
        }
    }

    /// Finish with an `ELSE` branch, so the result is never NULL for want of a
    /// matching arm.
    pub fn otherwise<V>(self, result: V) -> Expr<Merge<S, V::Sources>, T>
    where
        V: IntoExpr<T>,
        S: Concat<V::Sources>,
    {
        Expr::new(Node::Case {
            operand: None,
            arms: self.arms,
            otherwise: Some(Box::new(result.into_expr_node())),
        })
    }

    /// Finish without an `ELSE` branch.
    ///
    /// A row matching no arm yields NULL, which every column type here allows,
    /// so the result keeps the arms' own type.
    pub fn end(self) -> Expr<S, T> {
        Expr::new(Node::Case {
            operand: None,
            arms: self.arms,
            otherwise: None,
        })
    }
}

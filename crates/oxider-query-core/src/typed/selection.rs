//! Projections: one or more expressions usable in a SELECT list, GROUP BY, or
//! RETURNING clause.
//!
//! [`AnyExpr`] is "an expression of some type" - the erased view a clause needs
//! when it does not care what the expression evaluates to.
//!
//! [`SelectionIn`] is the tuple-friendly form. It checks each element's scope
//! separately, carrying one witness index per element, rather than merging the
//! element source sets first. That keeps the bounds flat: a six-column
//! projection needs six independent `ContainsAll` bounds instead of a nest of
//! `Concat` associated types, which is both faster to typecheck and far easier
//! to read in an error message.

use crate::ast::node::Node;
use crate::source::{Cons, ContainsAll, Nil};
use crate::typed::aggregate::Aggregate;
use crate::typed::expr::Expr;
use crate::typed::Column;

/// An expression whose Rust type is known but not constrained by the caller.
pub trait AnyExpr {
    /// The entities this expression references.
    type Sources;
    /// The Rust type it evaluates to.
    type Output;
    /// Lower into an AST node.
    fn into_any_node(self) -> Node;
}

impl<E, T> AnyExpr for Column<E, T> {
    type Sources = Cons<E, Nil>;
    type Output = T;
    fn into_any_node(self) -> Node {
        Node::Column(self.column_ref())
    }
}

impl<S, T> AnyExpr for Expr<S, T> {
    type Sources = S;
    type Output = T;
    fn into_any_node(self) -> Node {
        self.into_node()
    }
}

impl<S, T> AnyExpr for Aggregate<S, T> {
    type Sources = S;
    type Output = T;
    fn into_any_node(self) -> Node {
        self.into_node()
    }
}

/// A projection that is valid in the scope `S`.
///
/// `Idxs` is inferred: it is the tuple of membership witnesses proving each
/// element's entities are in scope. Callers never name it.
pub trait SelectionIn<S, Idxs> {
    /// Append this projection's expressions, in order.
    fn append_nodes(self, out: &mut Vec<Node>);
}

impl<S, A, I> SelectionIn<S, (I,)> for A
where
    A: AnyExpr,
    S: ContainsAll<A::Sources, I>,
{
    fn append_nodes(self, out: &mut Vec<Node>) {
        out.push(self.into_any_node());
    }
}

/// Generate a tuple projection impl of the given arity.
macro_rules! tuple_selection {
    ($($item:ident $idx:ident $field:tt),+) => {
        impl<S, $($item, $idx),+> SelectionIn<S, ($($idx,)+)> for ($($item,)+)
        where
            $($item: AnyExpr,)+
            $(S: ContainsAll<$item::Sources, $idx>,)+
        {
            fn append_nodes(self, out: &mut Vec<Node>) {
                $(out.push(self.$field.into_any_node());)+
            }
        }
    };
}

tuple_selection!(A IA 0, B IB 1);
tuple_selection!(A IA 0, B IB 1, C IC 2);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3, E IE 4);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5, G IG 6);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5, G IG 6, H IH 7);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5, G IG 6, H IH 7, I II 8);
tuple_selection!(A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5, G IG 6, H IH 7, I II 8, J IJ 9);
tuple_selection!(
    A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5, G IG 6, H IH 7, I II 8, J IJ 9, K IK 10
);
tuple_selection!(
    A IA 0, B IB 1, C IC 2, D ID 3, E IE 4, F IF 5, G IG 6, H IH 7, I II 8, J IJ 9, K IK 10,
    L IL 11
);

/// Conversions available on every expression, whatever kind it is.
///
/// [`Expr`] and [`Aggregate`] have these as inherent methods so they resolve
/// without an import; this trait is what puts them on [`Column`] too, which is
/// where they are most often wanted (`User::age.cast::<String>(..)`).
pub trait ExprExt: AnyExpr + Sized {
    /// `CAST(self AS <type>)`, with the type named per dialect.
    fn cast<U>(self, kind: crate::dialect::CastKind) -> Expr<Self::Sources, U> {
        Expr::new(Node::Cast {
            expr: Box::new(self.into_any_node()),
            kind,
        })
    }

    /// Reinterpret the Rust type without changing the SQL.
    ///
    /// The escape hatch for domain types the library cannot infer, such as an
    /// enum stored as text. It asserts a type the database is trusted to
    /// produce, so it is the one place the type-safety is only as good as the
    /// caller's claim.
    fn coerce<U>(self) -> Expr<Self::Sources, U> {
        Expr::new(self.into_any_node())
    }

    /// Name this expression in a SELECT list: `expr AS alias`.
    fn alias(self, name: &'static str) -> Expr<Self::Sources, Self::Output> {
        Expr::new(Node::Alias(Box::new(self.into_any_node()), name))
    }

    /// View this as a plain [`Expr`], which is what a variable holding a mixed
    /// set of expressions needs.
    fn into_expr(self) -> Expr<Self::Sources, Self::Output> {
        Expr::new(self.into_any_node())
    }
}

impl<X: AnyExpr> ExprExt for X {}

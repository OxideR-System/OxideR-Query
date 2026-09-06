//! Comparison, null tests, membership, and ordering.
//!
//! These are the operators QueryDSL puts on `SimpleExpression` and
//! `ComparableExpression`. Here they are two traits, split by the same line:
//! [`CompareOps`] needs only that the type can be bound as a value, while
//! [`OrderOps`] additionally needs it to be orderable, so `flag.gt(true)` does
//! not compile.
//!
//! Every operand position takes `impl IntoExpr<T>`, so one method covers both
//! "compare to a value" and "compare to another column" - the join-key case
//! QueryDSL needs a separate overload for.

// The `is_*` builders take `self` by value because the whole DSL is
// consuming: an operand is moved into the node it becomes. Taking `&self`
// would force a clone at every call site for no gain, since columns are `Copy`
// and expressions are single-use.
#![allow(clippy::wrong_self_convention)]

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::ast::query::OrderDir;
use crate::source::Concat;
use crate::typed::expr::{Expr, IntoExpr, Order, Predicate};
use crate::value::{Orderable, SqlType};

/// The source set produced by combining two operands.
pub type Merge<A, B> = <A as Concat<B>>::Out;

/// The source set produced by combining three operands.
///
/// Ternary operators - `BETWEEN`, and a `CASE` arm's condition and result -
/// merge right-associatively, so this names the shape their signatures repeat.
pub type Merge3<A, B, C> = Merge<A, Merge<B, C>>;

/// Build a binary predicate method taking one operand of the same type.
macro_rules! comparison {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name<R>(self, rhs: R) -> Predicate<Merge<Self::Sources, R::Sources>>
        where
            R: IntoExpr<T>,
            Self::Sources: Concat<R::Sources>,
        {
            Expr::new(Node::binary(
                $op,
                self.into_expr_node(),
                rhs.into_expr_node(),
            ))
        }
    };
}

/// Equality, null tests, and membership: available on every column type.
pub trait CompareOps<T>: IntoExpr<T> + Sized
where
    T: SqlType,
{
    comparison!(eq, Operator::Eq, "`self = rhs`");
    comparison!(ne, Operator::Ne, "`self <> rhs`");
    comparison!(
        is_not_distinct_from,
        Operator::IsNotDistinctFrom,
        "Null-safe equality: true when both sides are NULL."
    );
    comparison!(
        is_distinct_from,
        Operator::IsDistinctFrom,
        "Null-safe inequality: false when both sides are NULL."
    );

    /// Compare against another column of the same type.
    ///
    /// A more explicit spelling of [`eq`](CompareOps::eq) for join keys, kept
    /// because it reads well in an `ON` clause and matches QueryDSL's naming.
    fn eq_column<R>(self, rhs: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<T>,
        Self::Sources: Concat<R::Sources>,
    {
        self.eq(rhs)
    }

    /// `self IS NULL`
    fn is_null(self) -> Predicate<Self::Sources> {
        Expr::new(Node::unary(Operator::IsNull, self.into_expr_node()))
    }

    /// `self IS NOT NULL`
    fn is_not_null(self) -> Predicate<Self::Sources> {
        Expr::new(Node::unary(Operator::IsNotNull, self.into_expr_node()))
    }

    /// `self IN (v1, v2, ...)`
    ///
    /// An empty list renders as a predicate that is always false, which is what
    /// `IN ()` means but cannot be written in SQL.
    fn in_values<V, I>(self, values: I) -> Predicate<Self::Sources>
    where
        V: IntoExpr<T, Sources = crate::source::Nil>,
        I: IntoIterator<Item = V>,
    {
        membership(self, values, false)
    }

    /// `self NOT IN (v1, v2, ...)`
    ///
    /// An empty list renders as a predicate that is always true.
    fn not_in_values<V, I>(self, values: I) -> Predicate<Self::Sources>
    where
        V: IntoExpr<T, Sources = crate::source::Nil>,
        I: IntoIterator<Item = V>,
    {
        membership(self, values, true)
    }
}

/// Ordering comparisons and sort terms: available when the type is orderable.
pub trait OrderOps<T>: IntoExpr<T> + Sized
where
    T: SqlType + Orderable,
{
    comparison!(lt, Operator::Lt, "`self < rhs`");
    comparison!(le, Operator::Le, "`self <= rhs`");
    comparison!(gt, Operator::Gt, "`self > rhs`");
    comparison!(ge, Operator::Ge, "`self >= rhs`");

    /// `self BETWEEN low AND high`, inclusive on both ends.
    fn between<A, B>(
        self,
        low: A,
        high: B,
    ) -> Predicate<Merge3<Self::Sources, A::Sources, B::Sources>>
    where
        A: IntoExpr<T>,
        B: IntoExpr<T>,
        A::Sources: Concat<B::Sources>,
        Self::Sources: Concat<Merge<A::Sources, B::Sources>>,
    {
        Expr::new(Node::op(
            Operator::Between,
            [
                self.into_expr_node(),
                low.into_expr_node(),
                high.into_expr_node(),
            ],
        ))
    }

    /// `self NOT BETWEEN low AND high`.
    fn not_between<A, B>(
        self,
        low: A,
        high: B,
    ) -> Predicate<Merge3<Self::Sources, A::Sources, B::Sources>>
    where
        A: IntoExpr<T>,
        B: IntoExpr<T>,
        A::Sources: Concat<B::Sources>,
        Self::Sources: Concat<Merge<A::Sources, B::Sources>>,
    {
        Expr::new(Node::op(
            Operator::NotBetween,
            [
                self.into_expr_node(),
                low.into_expr_node(),
                high.into_expr_node(),
            ],
        ))
    }

    /// Sort by this expression, ascending.
    fn asc(self) -> Order<Self::Sources> {
        Order::new(self.into_expr_node(), OrderDir::Asc)
    }

    /// Sort by this expression, descending.
    fn desc(self) -> Order<Self::Sources> {
        Order::new(self.into_expr_node(), OrderDir::Desc)
    }
}

/// Build an `IN` / `NOT IN` predicate, handling the empty list.
fn membership<T, S, X, V, I>(lhs: X, values: I, negated: bool) -> Predicate<S>
where
    X: IntoExpr<T, Sources = S>,
    V: IntoExpr<T, Sources = crate::source::Nil>,
    I: IntoIterator<Item = V>,
{
    let items: Vec<Node> = values.into_iter().map(IntoExpr::into_expr_node).collect();
    if items.is_empty() {
        // `IN ()` is not valid SQL. An empty membership test is a constant, and
        // saying so keeps a dynamically built filter from silently matching
        // everything.
        let always = if negated { "1 = 1" } else { "1 = 0" };
        return Expr::new(Node::Keyword(always));
    }
    let op = if negated {
        Operator::NotIn
    } else {
        Operator::In
    };
    Expr::new(Node::binary(op, lhs.into_expr_node(), Node::Row(items)))
}

impl<E, T: SqlType> CompareOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType> CompareOps<T> for Expr<S, T> {}
impl<E, T: SqlType + Orderable> OrderOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType + Orderable> OrderOps<T> for Expr<S, T> {}

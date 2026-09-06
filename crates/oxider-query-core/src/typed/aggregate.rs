//! Aggregate functions.
//!
//! An [`Aggregate`] is a distinct type rather than a plain expression so that
//! `DISTINCT`, `FILTER (WHERE ...)`, an inner `ORDER BY`, and `OVER (...)` are
//! only offered where they mean something. It converts into an expression
//! everywhere one is accepted, so `count_all().gt(5)` works in a HAVING clause
//! exactly like a column comparison.

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::ast::query::OrderAst;
use crate::source::{Concat, Nil};
use crate::typed::expr::{Expr, IntoExpr, Order, Predicate};
use crate::typed::ops_compare::Merge;
use crate::value::{Numeric, Orderable, SqlType};
use core::marker::PhantomData;

/// An aggregate function call: `S` is the set of entities it references, `T`
/// its Rust result type.
pub struct Aggregate<S, T> {
    func: Operator,
    distinct: bool,
    args: Vec<Node>,
    order_by: Vec<OrderAst>,
    filter: Option<Box<Node>>,
    _marker: PhantomData<fn() -> (S, T)>,
}

impl<S, T> Aggregate<S, T> {
    /// Build an aggregate call from its operator and arguments.
    pub(crate) fn new(func: Operator, args: Vec<Node>) -> Self {
        Aggregate {
            func,
            distinct: false,
            args,
            order_by: Vec::new(),
            filter: None,
            _marker: PhantomData,
        }
    }

    /// Aggregate only distinct values: `COUNT(DISTINCT x)`.
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Aggregate only the rows matching a predicate.
    ///
    /// Renders as `FILTER (WHERE ...)` where the engine has it, and as a `CASE`
    /// folded into the argument where it does not, so the result is the same
    /// everywhere.
    pub fn filter_where<S2>(self, predicate: Predicate<S2>) -> Aggregate<Merge<S, S2>, T>
    where
        S: Concat<S2>,
    {
        Aggregate {
            func: self.func,
            distinct: self.distinct,
            args: self.args,
            order_by: self.order_by,
            filter: Some(Box::new(predicate.into_node())),
            _marker: PhantomData,
        }
    }

    /// Order the aggregated values, for aggregates where order matters such as
    /// string aggregation.
    pub fn order_by<S2>(mut self, term: Order<S2>) -> Aggregate<Merge<S, S2>, T>
    where
        S: Concat<S2>,
    {
        self.order_by.push(term.into_term());
        Aggregate {
            func: self.func,
            distinct: self.distinct,
            args: self.args,
            order_by: self.order_by,
            filter: self.filter,
            _marker: PhantomData,
        }
    }

    /// Name this aggregate in a SELECT list: `agg AS alias`.
    pub fn alias(self, name: &'static str) -> Expr<S, T> {
        Expr::new(Node::Alias(Box::new(self.into_node()), name))
    }

    /// Lower into an AST node.
    pub(crate) fn into_node(self) -> Node {
        Node::Aggregate {
            func: self.func,
            distinct: self.distinct,
            args: self.args,
            order_by: self.order_by,
            filter: self.filter,
        }
    }
}

impl<S, T> IntoExpr<T> for Aggregate<S, T> {
    type Sources = S;
    fn into_expr_node(self) -> Node {
        self.into_node()
    }
}

/// Aggregates available on any column type.
pub trait AggOps<T>: IntoExpr<T> + Sized
where
    T: SqlType,
{
    /// `COUNT(self)` - counts non-null values.
    fn count(self) -> Aggregate<Self::Sources, i64> {
        Aggregate::new(Operator::Count, vec![self.into_expr_node()])
    }

    /// `COUNT(DISTINCT self)`.
    fn count_distinct(self) -> Aggregate<Self::Sources, i64> {
        Aggregate::new(Operator::Count, vec![self.into_expr_node()]).distinct()
    }
}

/// Aggregates available on orderable column types.
pub trait OrderedAggOps<T>: IntoExpr<T> + Sized
where
    T: SqlType + Orderable,
{
    /// `MIN(self)`.
    fn min(self) -> Aggregate<Self::Sources, T> {
        Aggregate::new(Operator::Min, vec![self.into_expr_node()])
    }

    /// `MAX(self)`.
    fn max(self) -> Aggregate<Self::Sources, T> {
        Aggregate::new(Operator::Max, vec![self.into_expr_node()])
    }
}

/// Aggregates available on numeric column types.
pub trait NumericAggOps<T>: IntoExpr<T> + Sized
where
    T: SqlType + Numeric,
{
    /// `SUM(self)`, keeping the operand's type.
    fn sum(self) -> Aggregate<Self::Sources, T> {
        Aggregate::new(Operator::Sum, vec![self.into_expr_node()])
    }

    /// `AVG(self)`, always floating point.
    fn avg(self) -> Aggregate<Self::Sources, f64> {
        Aggregate::new(Operator::Avg, vec![self.into_expr_node()])
    }

    /// Sample standard deviation.
    fn std_dev(self) -> Aggregate<Self::Sources, f64> {
        Aggregate::new(Operator::StdDev, vec![self.into_expr_node()])
    }

    /// Population standard deviation.
    fn std_dev_pop(self) -> Aggregate<Self::Sources, f64> {
        Aggregate::new(Operator::StdDevPop, vec![self.into_expr_node()])
    }

    /// Sample variance.
    fn variance(self) -> Aggregate<Self::Sources, f64> {
        Aggregate::new(Operator::Variance, vec![self.into_expr_node()])
    }

    /// Population variance.
    fn var_pop(self) -> Aggregate<Self::Sources, f64> {
        Aggregate::new(Operator::VarPop, vec![self.into_expr_node()])
    }
}

/// `COUNT(*)` - counts rows, referencing no particular column.
pub fn count_all() -> Aggregate<Nil, i64> {
    Aggregate::new(Operator::CountAll, Vec::new())
}

/// `BOOL_AND(expr)` - true when every aggregated row is true.
pub fn bool_and<X>(expr: X) -> Aggregate<X::Sources, bool>
where
    X: IntoExpr<bool>,
{
    Aggregate::new(Operator::BoolAnd, vec![expr.into_expr_node()])
}

/// `BOOL_OR(expr)` - true when any aggregated row is true.
pub fn bool_or<X>(expr: X) -> Aggregate<X::Sources, bool>
where
    X: IntoExpr<bool>,
{
    Aggregate::new(Operator::BoolOr, vec![expr.into_expr_node()])
}

/// Concatenate the aggregated strings, separated by `separator`.
///
/// Renders as `STRING_AGG` on PostgreSQL and `GROUP_CONCAT` on MySQL and
/// SQLite.
pub fn group_concat<X>(expr: X, separator: impl AsRef<str>) -> Aggregate<X::Sources, String>
where
    X: IntoExpr<String>,
{
    Aggregate::new(
        Operator::GroupConcat,
        vec![
            expr.into_expr_node(),
            Node::Param(crate::value::Value::Text(separator.as_ref().to_string())),
        ],
    )
}

impl<E, T: SqlType> AggOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType> AggOps<T> for Expr<S, T> {}
impl<E, T: SqlType + Orderable> OrderedAggOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType + Orderable> OrderedAggOps<T> for Expr<S, T> {}
impl<E, T: SqlType + Numeric> NumericAggOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType + Numeric> NumericAggOps<T> for Expr<S, T> {}

// An aggregate is an expression wherever one is expected, so HAVING clauses and
// arithmetic over aggregates work without a conversion step.
impl<S, T: SqlType> crate::typed::ops_compare::CompareOps<T> for Aggregate<S, T> {}
impl<S, T: SqlType + Orderable> crate::typed::ops_compare::OrderOps<T> for Aggregate<S, T> {}
impl<S, T: SqlType + Numeric> crate::typed::ops_math::MathOps<T> for Aggregate<S, T> {}

//! Typed aggregate expressions (`COUNT`, `SUM`, `AVG`, `MIN`, `MAX`).
//!
//! An [`Aggregate<S, T>`] carries the entities it references (`S`, for the same
//! in-scope checking as columns) and its result Rust type (`T`, for value
//! binding in HAVING comparisons and, later, projection). Aggregates are built
//! with the free functions here and used in `select`, `group_by` is for columns,
//! and HAVING via comparison methods:
//!
//! ```ignore
//! User::query()
//!     .select((User::department_id, count_all()))
//!     .group_by(User::department_id)
//!     .having(count_all().gt(5))
//! ```

use crate::column::Column;
use crate::expr::{AggFunc, BinOp, Expr};
use crate::predicate::Predicate;
use crate::query::SelectItem;
use crate::source::{Cons, Nil};
use crate::value::{Numeric, Orderable, ToSqlValue};
use core::marker::PhantomData;

/// A typed aggregate function call. `S` is the referenced source set, `T` the
/// Rust result type.
pub struct Aggregate<S, T> {
    expr: Expr,
    _marker: PhantomData<fn() -> (S, T)>,
}

impl<S, T> Aggregate<S, T> {
    fn new(func: AggFunc, arg: Option<Expr>) -> Self {
        Aggregate {
            expr: Expr::Aggregate {
                func,
                arg: arg.map(Box::new),
            },
            _marker: PhantomData,
        }
    }
}

/// The source set contributed by an aggregate over a single column of `E`.
type Over<E> = Cons<E, Nil>;

/// `COUNT(*)` - counts rows, references no particular column.
pub fn count_all() -> Aggregate<Nil, i64> {
    Aggregate::new(AggFunc::Count, None)
}

/// `COUNT(column)` - counts non-null values of the column.
pub fn count<E, T>(column: Column<E, T>) -> Aggregate<Over<E>, i64> {
    Aggregate::new(AggFunc::Count, Some(column_expr(&column)))
}

/// `SUM(column)` - only on numeric columns. Result keeps the column's type.
pub fn sum<E, T: Numeric>(column: Column<E, T>) -> Aggregate<Over<E>, T> {
    Aggregate::new(AggFunc::Sum, Some(column_expr(&column)))
}

/// `AVG(column)` - only on numeric columns. Result is floating point.
pub fn avg<E, T: Numeric>(column: Column<E, T>) -> Aggregate<Over<E>, f64> {
    Aggregate::new(AggFunc::Avg, Some(column_expr(&column)))
}

/// `MIN(column)` - only on orderable columns. Result keeps the column's type.
pub fn min<E, T: Orderable>(column: Column<E, T>) -> Aggregate<Over<E>, T> {
    Aggregate::new(AggFunc::Min, Some(column_expr(&column)))
}

/// `MAX(column)` - only on orderable columns. Result keeps the column's type.
pub fn max<E, T: Orderable>(column: Column<E, T>) -> Aggregate<Over<E>, T> {
    Aggregate::new(AggFunc::Max, Some(column_expr(&column)))
}

fn column_expr<E, T>(column: &Column<E, T>) -> Expr {
    Expr::Column {
        table: column.table,
        name: column.name,
    }
}

/// HAVING comparisons: compare an aggregate against a value. Available when the
/// result type both binds a value and is orderable (all aggregate results are).
impl<S, T: ToSqlValue + Orderable> Aggregate<S, T> {
    /// `self = value`
    pub fn eq<V: Into<T>>(self, value: V) -> Predicate<S> {
        self.compare(BinOp::Eq, value)
    }
    /// `self <> value`
    pub fn ne<V: Into<T>>(self, value: V) -> Predicate<S> {
        self.compare(BinOp::Ne, value)
    }
    /// `self > value`
    pub fn gt<V: Into<T>>(self, value: V) -> Predicate<S> {
        self.compare(BinOp::Gt, value)
    }
    /// `self >= value`
    pub fn ge<V: Into<T>>(self, value: V) -> Predicate<S> {
        self.compare(BinOp::Ge, value)
    }
    /// `self < value`
    pub fn lt<V: Into<T>>(self, value: V) -> Predicate<S> {
        self.compare(BinOp::Lt, value)
    }
    /// `self <= value`
    pub fn le<V: Into<T>>(self, value: V) -> Predicate<S> {
        self.compare(BinOp::Le, value)
    }

    fn compare<V: Into<T>>(self, op: BinOp, value: V) -> Predicate<S> {
        let bound = value.into().to_sql_value();
        Predicate::new(Expr::Binary {
            op,
            lhs: Box::new(self.expr),
            rhs: Box::new(Expr::Param(bound)),
        })
    }
}

impl<S, T> SelectItem for Aggregate<S, T> {
    type Sources = S;
    type Output = T;
    fn into_select_expr(self) -> Expr {
        self.expr
    }
}

//! Date and time functions, available on temporal expressions.
//!
//! Every field extraction returns `i32` and every truncation returns the
//! operand's own type, so a query reads the same on any engine even though the
//! generated SQL differs a lot (see the dialect tables).
//!
//! `day_of_week` is normalised to 1 for Sunday through 7 for Saturday on every
//! dialect, matching QueryDSL and MySQL rather than leaking PostgreSQL's
//! zero-based `DOW`.

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::typed::expr::{Expr, IntoExpr};
use crate::value::{SqlType, Temporal, Value};

/// Build a field-extraction method returning an integer.
macro_rules! field {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name(self) -> Expr<Self::Sources, i32> {
            Expr::new(Node::unary($op, self.into_expr_node()))
        }
    };
}

/// Build a method that shifts a temporal value by a number of units.
macro_rules! shift {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name(self, amount: i64) -> Expr<Self::Sources, T> {
            Expr::new(Node::op(
                $op,
                [self.into_expr_node(), Node::Param(Value::Int(amount))],
            ))
        }
    };
}

/// Build a truncation method that keeps the operand type.
macro_rules! truncate {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name(self) -> Expr<Self::Sources, T> {
            Expr::new(Node::unary($op, self.into_expr_node()))
        }
    };
}

/// Date and time operations on a temporal expression.
pub trait TemporalOps<T>: IntoExpr<T> + Sized
where
    T: SqlType + Temporal,
{
    field!(year, Operator::Year, "The year.");
    field!(month, Operator::Month, "The month, 1 through 12.");
    field!(day, Operator::DayOfMonth, "The day of the month.");
    field!(hour, Operator::Hour, "The hour, 0 through 23.");
    field!(minute, Operator::Minute, "The minute.");
    field!(second, Operator::Second, "The second.");
    field!(millisecond, Operator::Millisecond, "The millisecond.");
    field!(week, Operator::Week, "The week of the year.");
    field!(
        day_of_week,
        Operator::DayOfWeek,
        "The day of the week, 1 for Sunday through 7 for Saturday."
    );
    field!(day_of_year, Operator::DayOfYear, "The day of the year.");
    field!(
        year_month,
        Operator::YearMonth,
        "Year and month combined as `YYYYMM`."
    );
    field!(
        year_week,
        Operator::YearWeek,
        "Year and week combined as `YYYYWW`."
    );

    shift!(add_years, Operator::AddYears, "Add a number of years.");
    shift!(add_months, Operator::AddMonths, "Add a number of months.");
    shift!(add_weeks, Operator::AddWeeks, "Add a number of weeks.");
    shift!(add_days, Operator::AddDays, "Add a number of days.");
    shift!(add_hours, Operator::AddHours, "Add a number of hours.");
    shift!(
        add_minutes,
        Operator::AddMinutes,
        "Add a number of minutes."
    );
    shift!(
        add_seconds,
        Operator::AddSeconds,
        "Add a number of seconds."
    );

    truncate!(
        truncate_to_year,
        Operator::TruncYear,
        "Truncate to the start of the year."
    );
    truncate!(
        truncate_to_month,
        Operator::TruncMonth,
        "Truncate to the start of the month."
    );
    truncate!(
        truncate_to_week,
        Operator::TruncWeek,
        "Truncate to the start of the week."
    );
    truncate!(truncate_to_day, Operator::TruncDay, "Truncate to midnight.");
    truncate!(
        truncate_to_hour,
        Operator::TruncHour,
        "Truncate to the start of the hour."
    );
    truncate!(
        truncate_to_minute,
        Operator::TruncMinute,
        "Truncate to the start of the minute."
    );
    truncate!(
        truncate_to_second,
        Operator::TruncSecond,
        "Truncate to the start of the second."
    );

    /// Whole days from `self` to `other`, truncated toward zero.
    fn diff_days<R>(self, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        self.difference(Operator::DiffDays, other)
    }

    /// Whole years from `self` to `other`.
    fn diff_years<R>(self, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        self.difference(Operator::DiffYears, other)
    }

    /// Whole months from `self` to `other`.
    fn diff_months<R>(self, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        self.difference(Operator::DiffMonths, other)
    }

    /// Whole hours from `self` to `other`.
    fn diff_hours<R>(self, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        self.difference(Operator::DiffHours, other)
    }

    /// Whole minutes from `self` to `other`.
    fn diff_minutes<R>(self, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        self.difference(Operator::DiffMinutes, other)
    }

    /// Whole seconds from `self` to `other`.
    fn diff_seconds<R>(self, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        self.difference(Operator::DiffSeconds, other)
    }

    /// Drop the time part, keeping the date.
    fn date(self) -> Expr<Self::Sources, T> {
        Expr::new(Node::unary(Operator::DateOf, self.into_expr_node()))
    }

    /// Apply a two-argument difference operator.
    #[doc(hidden)]
    fn difference<R>(self, op: Operator, other: R) -> Expr<Self::Sources, i64>
    where
        R: IntoExpr<T, Sources = crate::source::Nil>,
    {
        Expr::new(Node::binary(
            op,
            self.into_expr_node(),
            other.into_expr_node(),
        ))
    }
}

impl<E, T: SqlType + Temporal> TemporalOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType + Temporal> TemporalOps<T> for Expr<S, T> {}

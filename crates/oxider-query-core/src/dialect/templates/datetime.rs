//! Templates for date and time operators.
//!
//! No two engines agree here, which is exactly why the library is worth having:
//! `User::created_at.year()` should mean the same thing whether it runs on
//! PostgreSQL, MySQL, or SQLite, and this table is where that promise is kept.
//!
//! Two conventions are normalised across dialects rather than passed through:
//!
//! - `day_of_week` returns 1 for Sunday through 7 for Saturday, matching
//!   QueryDSL and MySQL. PostgreSQL's `DOW` (0 for Sunday) and SQLite's `%w`
//!   are shifted by one here so the value does not depend on the backend.
//! - The `diff_*` operators return whole units from the first argument to the
//!   second, truncated toward zero.
//!
//! An argument that appears twice in a template is rendered twice, so a bound
//! value is sent twice. That is correct but means the expression is evaluated
//! twice by the engine; keep repeated arguments to cheap expressions.

use crate::ast::operator::Operator;
use crate::dialect::template::{t, Elem::Arg as A, Elem::Lit as L, Template};

/// The ANSI template for a date/time operator.
///
/// The baseline is the SQL-standard `EXTRACT`/`INTERVAL` vocabulary, which
/// PostgreSQL implements almost exactly.
pub(crate) fn ansi(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        CurrentDate => t![L("CURRENT_DATE")],
        CurrentTime => t![L("CURRENT_TIME")],
        CurrentTimestamp => t![L("CURRENT_TIMESTAMP")],
        DateOf => t![L("CAST("), A(0), L(" AS DATE)")],

        Year => t![L("EXTRACT(YEAR FROM "), A(0), L(")")],
        Month => t![L("EXTRACT(MONTH FROM "), A(0), L(")")],
        DayOfMonth => t![L("EXTRACT(DAY FROM "), A(0), L(")")],
        Hour => t![L("EXTRACT(HOUR FROM "), A(0), L(")")],
        Minute => t![L("EXTRACT(MINUTE FROM "), A(0), L(")")],
        Second => t![L("EXTRACT(SECOND FROM "), A(0), L(")")],
        Millisecond => t![L("EXTRACT(MILLISECONDS FROM "), A(0), L(")")],
        Week => t![L("EXTRACT(WEEK FROM "), A(0), L(")")],
        DayOfWeek => t![L("(EXTRACT(DOW FROM "), A(0), L(") + 1)")],
        DayOfYear => t![L("EXTRACT(DOY FROM "), A(0), L(")")],
        YearMonth => t![
            L("(EXTRACT(YEAR FROM "),
            A(0),
            L(") * 100 + EXTRACT(MONTH FROM "),
            A(0),
            L("))")
        ],
        YearWeek => t![
            L("(EXTRACT(YEAR FROM "),
            A(0),
            L(") * 100 + EXTRACT(WEEK FROM "),
            A(0),
            L("))")
        ],

        AddYears => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 year'))")],
        AddMonths => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 month'))")],
        AddWeeks => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 week'))")],
        AddDays => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 day'))")],
        AddHours => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 hour'))")],
        AddMinutes => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 minute'))")],
        AddSeconds => t![L("("), A(0), L(" + ("), A(1), L(" * INTERVAL '1 second'))")],

        DiffYears => t![
            L("(EXTRACT(YEAR FROM "),
            A(1),
            L(") - EXTRACT(YEAR FROM "),
            A(0),
            L("))")
        ],
        DiffMonths => t![
            L("((EXTRACT(YEAR FROM "),
            A(1),
            L(") - EXTRACT(YEAR FROM "),
            A(0),
            L(")) * 12 + (EXTRACT(MONTH FROM "),
            A(1),
            L(") - EXTRACT(MONTH FROM "),
            A(0),
            L(")))")
        ],
        DiffWeeks => t![
            L("CAST(EXTRACT(EPOCH FROM ("),
            A(1),
            L(" - "),
            A(0),
            L(")) / 604800 AS INTEGER)")
        ],
        DiffDays => t![
            L("CAST(EXTRACT(EPOCH FROM ("),
            A(1),
            L(" - "),
            A(0),
            L(")) / 86400 AS INTEGER)")
        ],
        DiffHours => t![
            L("CAST(EXTRACT(EPOCH FROM ("),
            A(1),
            L(" - "),
            A(0),
            L(")) / 3600 AS INTEGER)")
        ],
        DiffMinutes => t![
            L("CAST(EXTRACT(EPOCH FROM ("),
            A(1),
            L(" - "),
            A(0),
            L(")) / 60 AS INTEGER)")
        ],
        DiffSeconds => t![
            L("CAST(EXTRACT(EPOCH FROM ("),
            A(1),
            L(" - "),
            A(0),
            L(")) AS INTEGER)")
        ],

        TruncYear => t![L("DATE_TRUNC('year', "), A(0), L(")")],
        TruncMonth => t![L("DATE_TRUNC('month', "), A(0), L(")")],
        TruncWeek => t![L("DATE_TRUNC('week', "), A(0), L(")")],
        TruncDay => t![L("DATE_TRUNC('day', "), A(0), L(")")],
        TruncHour => t![L("DATE_TRUNC('hour', "), A(0), L(")")],
        TruncMinute => t![L("DATE_TRUNC('minute', "), A(0), L(")")],
        TruncSecond => t![L("DATE_TRUNC('second', "), A(0), L(")")],

        _ => return None,
    })
}

/// PostgreSQL overrides: none, PostgreSQL is the baseline for this family.
pub(crate) fn postgres(_op: Operator) -> Option<Template> {
    None
}

/// MySQL overrides.
///
/// MySQL prefers named functions over `EXTRACT`, spells interval arithmetic
/// with `DATE_ADD`, and has no `DATE_TRUNC` at all.
pub(crate) fn mysql(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        DateOf => t![L("DATE("), A(0), L(")")],
        Year => t![L("YEAR("), A(0), L(")")],
        Month => t![L("MONTH("), A(0), L(")")],
        DayOfMonth => t![L("DAYOFMONTH("), A(0), L(")")],
        Hour => t![L("HOUR("), A(0), L(")")],
        Minute => t![L("MINUTE("), A(0), L(")")],
        Second => t![L("SECOND("), A(0), L(")")],
        Millisecond => t![L("(MICROSECOND("), A(0), L(") DIV 1000)")],
        Week => t![L("WEEK("), A(0), L(")")],
        DayOfWeek => t![L("DAYOFWEEK("), A(0), L(")")],
        DayOfYear => t![L("DAYOFYEAR("), A(0), L(")")],
        YearMonth => t![L("EXTRACT(YEAR_MONTH FROM "), A(0), L(")")],
        YearWeek => t![L("YEARWEEK("), A(0), L(")")],

        AddYears => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" YEAR)")],
        AddMonths => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" MONTH)")],
        AddWeeks => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" WEEK)")],
        AddDays => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" DAY)")],
        AddHours => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" HOUR)")],
        AddMinutes => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" MINUTE)")],
        AddSeconds => t![L("DATE_ADD("), A(0), L(", INTERVAL "), A(1), L(" SECOND)")],

        DiffYears => t![L("TIMESTAMPDIFF(YEAR, "), A(0), L(", "), A(1), L(")")],
        DiffMonths => t![L("TIMESTAMPDIFF(MONTH, "), A(0), L(", "), A(1), L(")")],
        DiffWeeks => t![L("TIMESTAMPDIFF(WEEK, "), A(0), L(", "), A(1), L(")")],
        DiffDays => t![L("TIMESTAMPDIFF(DAY, "), A(0), L(", "), A(1), L(")")],
        DiffHours => t![L("TIMESTAMPDIFF(HOUR, "), A(0), L(", "), A(1), L(")")],
        DiffMinutes => t![L("TIMESTAMPDIFF(MINUTE, "), A(0), L(", "), A(1), L(")")],
        DiffSeconds => t![L("TIMESTAMPDIFF(SECOND, "), A(0), L(", "), A(1), L(")")],

        TruncYear => t![L("MAKEDATE(YEAR("), A(0), L("), 1)")],
        TruncMonth => t![
            L("DATE_SUB(DATE("),
            A(0),
            L("), INTERVAL DAYOFMONTH("),
            A(0),
            L(") - 1 DAY)")
        ],
        TruncWeek => t![
            L("DATE_SUB(DATE("),
            A(0),
            L("), INTERVAL WEEKDAY("),
            A(0),
            L(") DAY)")
        ],
        TruncDay => t![L("DATE("), A(0), L(")")],
        TruncHour => t![L("DATE_FORMAT("), A(0), L(", '%Y-%m-%d %H:00:00')")],
        TruncMinute => t![L("DATE_FORMAT("), A(0), L(", '%Y-%m-%d %H:%i:00')")],
        TruncSecond => t![L("DATE_FORMAT("), A(0), L(", '%Y-%m-%d %H:%i:%s')")],

        _ => return None,
    })
}

/// SQLite overrides.
///
/// SQLite has no date type; timestamps are text, and every field extraction
/// goes through `STRFTIME`, which returns text, so each is cast back to an
/// integer to keep the Rust-side type honest.
pub(crate) fn sqlite(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        DateOf => t![L("DATE("), A(0), L(")")],
        Year => t![L("CAST(STRFTIME('%Y', "), A(0), L(") AS INTEGER)")],
        Month => t![L("CAST(STRFTIME('%m', "), A(0), L(") AS INTEGER)")],
        DayOfMonth => t![L("CAST(STRFTIME('%d', "), A(0), L(") AS INTEGER)")],
        Hour => t![L("CAST(STRFTIME('%H', "), A(0), L(") AS INTEGER)")],
        Minute => t![L("CAST(STRFTIME('%M', "), A(0), L(") AS INTEGER)")],
        Second => t![L("CAST(STRFTIME('%S', "), A(0), L(") AS INTEGER)")],
        Millisecond => t![
            L("CAST((CAST(STRFTIME('%f', "),
            A(0),
            L(") AS REAL) - CAST(STRFTIME('%S', "),
            A(0),
            L(") AS REAL)) * 1000 AS INTEGER)")
        ],
        Week => t![L("CAST(STRFTIME('%W', "), A(0), L(") AS INTEGER)")],
        DayOfWeek => t![L("(CAST(STRFTIME('%w', "), A(0), L(") AS INTEGER) + 1)")],
        DayOfYear => t![L("CAST(STRFTIME('%j', "), A(0), L(") AS INTEGER)")],
        YearMonth => t![L("CAST(STRFTIME('%Y%m', "), A(0), L(") AS INTEGER)")],
        YearWeek => t![L("CAST(STRFTIME('%Y%W', "), A(0), L(") AS INTEGER)")],

        AddYears => t![L("DATETIME("), A(0), L(", "), A(1), L(" || ' years')")],
        AddMonths => t![L("DATETIME("), A(0), L(", "), A(1), L(" || ' months')")],
        AddWeeks => t![L("DATETIME("), A(0), L(", ("), A(1), L(" * 7) || ' days')")],
        AddDays => t![L("DATETIME("), A(0), L(", "), A(1), L(" || ' days')")],
        AddHours => t![L("DATETIME("), A(0), L(", "), A(1), L(" || ' hours')")],
        AddMinutes => t![L("DATETIME("), A(0), L(", "), A(1), L(" || ' minutes')")],
        AddSeconds => t![L("DATETIME("), A(0), L(", "), A(1), L(" || ' seconds')")],

        DiffYears => t![
            L("(CAST(STRFTIME('%Y', "),
            A(1),
            L(") AS INTEGER) - CAST(STRFTIME('%Y', "),
            A(0),
            L(") AS INTEGER))")
        ],
        DiffMonths => t![
            L("((CAST(STRFTIME('%Y', "),
            A(1),
            L(") AS INTEGER) - CAST(STRFTIME('%Y', "),
            A(0),
            L(") AS INTEGER)) * 12 + (CAST(STRFTIME('%m', "),
            A(1),
            L(") AS INTEGER) - CAST(STRFTIME('%m', "),
            A(0),
            L(") AS INTEGER)))")
        ],
        DiffWeeks => t![
            L("CAST((JULIANDAY("),
            A(1),
            L(") - JULIANDAY("),
            A(0),
            L(")) / 7 AS INTEGER)")
        ],
        DiffDays => t![
            L("CAST(JULIANDAY("),
            A(1),
            L(") - JULIANDAY("),
            A(0),
            L(") AS INTEGER)")
        ],
        DiffHours => t![
            L("CAST((JULIANDAY("),
            A(1),
            L(") - JULIANDAY("),
            A(0),
            L(")) * 24 AS INTEGER)")
        ],
        DiffMinutes => t![
            L("CAST((JULIANDAY("),
            A(1),
            L(") - JULIANDAY("),
            A(0),
            L(")) * 1440 AS INTEGER)")
        ],
        DiffSeconds => t![
            L("CAST((JULIANDAY("),
            A(1),
            L(") - JULIANDAY("),
            A(0),
            L(")) * 86400 AS INTEGER)")
        ],

        TruncYear => t![L("DATE("), A(0), L(", 'start of year')")],
        TruncMonth => t![L("DATE("), A(0), L(", 'start of month')")],
        TruncWeek => t![L("DATE("), A(0), L(", 'weekday 0', '-7 days')")],
        TruncDay => t![L("DATE("), A(0), L(")")],
        TruncHour => t![L("STRFTIME('%Y-%m-%d %H:00:00', "), A(0), L(")")],
        TruncMinute => t![L("STRFTIME('%Y-%m-%d %H:%M:00', "), A(0), L(")")],
        TruncSecond => t![L("STRFTIME('%Y-%m-%d %H:%M:%S', "), A(0), L(")")],

        _ => return None,
    })
}

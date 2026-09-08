//! Bound parameter values, Rust-to-value conversion, and the type-capability
//! markers that gate which operators a column exposes.
//!
//! The marker traits mirror QueryDSL's expression class hierarchy: where Java
//! gates operators by which subclass a path is (`NumberExpression` exposes
//! arithmetic, `StringExpression` exposes `like`), OxideR gates them by a trait
//! bound on the column's Rust type. The mapping is one-to-one:
//!
//! | QueryDSL class | OxideR bound |
//! |----------------|--------------|
//! | `SimpleExpression` | [`SqlType`] |
//! | `ComparableExpression` | [`Orderable`] |
//! | `NumberExpression` | [`Numeric`] |
//! | `StringExpression` | `T = String` |
//! | `DateTimeExpression` | [`Temporal`] |
//! | `BooleanExpression` | `T = bool` |

/// A concrete value bound as a query parameter.
///
/// Self-contained so the core has no database dependency; the execution layer
/// bridges these to `sqlx` encoding.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Boolean parameter.
    Bool(bool),
    /// Signed integer parameter (widened to i64).
    Int(i64),
    /// Floating point parameter (widened to f64).
    Real(f64),
    /// Text parameter.
    Text(String),
    /// Binary parameter.
    Bytes(Vec<u8>),
    /// Calendar date, as `YYYY-MM-DD`.
    Date(String),
    /// Wall-clock time, as `HH:MM:SS[.fff]`.
    Time(String),
    /// Timestamp, as `YYYY-MM-DD HH:MM:SS[.fff]`.
    DateTime(String),
    /// SQL NULL.
    Null,
}

/// Converts a Rust value into a bound [`Value`].
///
/// Implemented for scalar built-ins; downstream crates implement it for their
/// own domain types (for example an enum stored as text or integer). Anything
/// implementing this can be passed wherever an expression of its type is
/// expected, and becomes a bind parameter rather than inlined SQL text.
pub trait ToSqlValue {
    /// Produce the bound value for this Rust value.
    fn to_sql_value(&self) -> Value;
}

macro_rules! to_sql_value {
    ($($ty:ty => |$v:ident| $body:expr),* $(,)?) => {
        $(impl ToSqlValue for $ty {
            fn to_sql_value(&self) -> Value {
                let $v = self;
                $body
            }
        })*
    };
}

to_sql_value! {
    i8 => |v| Value::Int(*v as i64),
    i16 => |v| Value::Int(*v as i64),
    i32 => |v| Value::Int(*v as i64),
    i64 => |v| Value::Int(*v),
    u8 => |v| Value::Int(*v as i64),
    u16 => |v| Value::Int(*v as i64),
    u32 => |v| Value::Int(*v as i64),
    f32 => |v| Value::Real(*v as f64),
    f64 => |v| Value::Real(*v),
    bool => |v| Value::Bool(*v),
    String => |v| Value::Text(v.clone()),
    Vec<u8> => |v| Value::Bytes(v.clone()),
}

impl<T: ToSqlValue> ToSqlValue for Option<T> {
    fn to_sql_value(&self) -> Value {
        match self {
            Some(v) => v.to_sql_value(),
            None => Value::Null,
        }
    }
}

/// Conversions into a bound value, for the places that take a value directly
/// rather than an expression - notably the raw-SQL escape hatch.
///
/// Separate from [`ToSqlValue`] on purpose: `ToSqlValue` drives operand type
/// checking, and adding `&str` to it would make every position whose expected
/// type is still open ambiguous between `&str` and `String`.
macro_rules! value_from {
    ($($ty:ty => |$v:ident| $body:expr),* $(,)?) => {
        $(impl From<$ty> for Value {
            fn from(value: $ty) -> Value {
                let $v = value;
                $body
            }
        })*
    };
}

value_from! {
    bool => |v| Value::Bool(v),
    i8 => |v| Value::Int(v as i64),
    i16 => |v| Value::Int(v as i64),
    i32 => |v| Value::Int(v as i64),
    i64 => |v| Value::Int(v),
    u8 => |v| Value::Int(v as i64),
    u16 => |v| Value::Int(v as i64),
    u32 => |v| Value::Int(v as i64),
    f32 => |v| Value::Real(v as f64),
    f64 => |v| Value::Real(v),
    String => |v| Value::Text(v),
    &str => |v| Value::Text(v.to_string()),
    Vec<u8> => |v| Value::Bytes(v),
}

/// `None` becomes SQL NULL.
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Value {
        match value {
            Some(inner) => inner.into(),
            None => Value::Null,
        }
    }
}

/// Marker for Rust types usable as a SQL column/expression type.
///
/// The base capability, matching QueryDSL's `SimpleExpression`: equality,
/// null tests, `IN`, and use as a projected column. Every type that can be
/// bound as a value is one, plus wrapper types the value layer understands.
pub trait SqlType {}

/// Marker for Rust types whose expressions support ordering comparisons
/// (`<`, `>`, `<=`, `>=`, `BETWEEN`, `ORDER BY`, `MIN`/`MAX`).
///
/// Matches QueryDSL's `ComparableExpression`. Numbers, strings and temporals
/// qualify; booleans and byte strings do not.
pub trait Orderable: SqlType {}

/// Marker for numeric Rust types, gating arithmetic and the arithmetic
/// aggregates `SUM`/`AVG`. Matches QueryDSL's `NumberExpression`.
pub trait Numeric: Orderable {}

/// Marker for date/time Rust types, gating the date-part extraction and date
/// arithmetic operators. Matches QueryDSL's `TemporalExpression`.
pub trait Temporal: Orderable {}

macro_rules! mark {
    ($trait:ident for $($ty:ty),* $(,)?) => { $(impl $trait for $ty {})* };
}

mark!(SqlType for i8, i16, i32, i64, u8, u16, u32, f32, f64, bool, String, Vec<u8>);
mark!(Orderable for i8, i16, i32, i64, u8, u16, u32, f32, f64, String);
mark!(Numeric for i8, i16, i32, i64, u8, u16, u32, f32, f64);

/// A nullable column type is usable wherever its inner type is, so `Option<T>`
/// inherits the same capabilities.
impl<T: SqlType> SqlType for Option<T> {}
impl<T: Orderable> Orderable for Option<T> {}
impl<T: Numeric> Numeric for Option<T> {}
impl<T: Temporal> Temporal for Option<T> {}

/// Temporal types, when the `chrono` feature is on.
///
/// Values bind as ISO-8601 text rather than as a driver-native date type. Every
/// engine parses that form in a date context, the text is identical across the
/// three dialects, and it keeps the core free of a driver dependency - the
/// execution layer binds a `Value::Date` as whatever its driver prefers.
/// The textual forms [`Value`]'s temporal variants hold.
///
/// A temporal is stored as text because the AST has to stay independent of any
/// date library, but an execution backend then has to read it back: PostgreSQL
/// carries a type per bound parameter and rejects text where a `date` is
/// expected, so its backend parses these before binding. Writing and reading
/// are therefore two halves of one contract, and it only holds if both halves
/// name the same format, which is why these are public rather than private to
/// the writing side.
pub mod formats {
    /// `YYYY-MM-DD`.
    pub const DATE: &str = "%Y-%m-%d";
    /// `HH:MM:SS`, with a fractional part only when there is one.
    pub const TIME: &str = "%H:%M:%S%.f";
    /// `YYYY-MM-DD HH:MM:SS`, the form every engine accepts unquoted.
    pub const DATE_TIME: &str = "%Y-%m-%d %H:%M:%S%.f";
    /// An instant, with an explicit offset so an engine storing it in a zoned
    /// column does not reinterpret it in the session's own zone.
    ///
    /// The offset is `%:z` rather than a literal `+00:00`, which formats
    /// identically for a UTC instant but also parses: chrono will not read a
    /// zoned timestamp from a format whose offset is plain text, so a literal
    /// here would be writable and not readable.
    pub const DATE_TIME_UTC: &str = "%Y-%m-%d %H:%M:%S%.f%:z";
}

#[cfg(feature = "chrono")]
mod temporal {
    use super::{Orderable, SqlType, Temporal, ToSqlValue, Value};
    use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

    use super::formats::{DATE, DATE_TIME, DATE_TIME_UTC, TIME};

    impl ToSqlValue for NaiveDate {
        fn to_sql_value(&self) -> Value {
            Value::Date(self.format(DATE).to_string())
        }
    }

    impl ToSqlValue for NaiveTime {
        fn to_sql_value(&self) -> Value {
            Value::Time(self.format(TIME).to_string())
        }
    }

    impl ToSqlValue for NaiveDateTime {
        fn to_sql_value(&self) -> Value {
            Value::DateTime(self.format(DATE_TIME).to_string())
        }
    }

    /// An instant binds with an explicit `+00:00`, so an engine storing it in a
    /// zoned column does not reinterpret it in the session's own zone.
    impl ToSqlValue for DateTime<Utc> {
        fn to_sql_value(&self) -> Value {
            Value::DateTime(self.format(DATE_TIME_UTC).to_string())
        }
    }

    macro_rules! temporal {
        ($($ty:ty),* $(,)?) => {
            $(impl SqlType for $ty {}
              impl Orderable for $ty {}
              impl Temporal for $ty {})*
        };
    }

    temporal!(NaiveDate, NaiveTime, NaiveDateTime, DateTime<Utc>);
}

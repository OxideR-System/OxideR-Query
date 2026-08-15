//! Bound parameter values, Rust-to-value conversion, and ordering capability.

/// A concrete value bound as a query parameter.
///
/// Self-contained for now so the core has no database dependency; the future
/// exec layer bridges these to `sqlx` encoding.
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
    /// SQL NULL.
    Null,
}

/// Converts a Rust value into a bound [`Value`].
///
/// Implemented for scalar built-ins; downstream crates implement it for their
/// own domain types (for example an enum stored as text or integer).
pub trait ToSqlValue {
    /// Produce the bound value for this Rust value.
    fn to_sql_value(&self) -> Value;
}

impl ToSqlValue for i16 {
    fn to_sql_value(&self) -> Value {
        Value::Int(*self as i64)
    }
}
impl ToSqlValue for i32 {
    fn to_sql_value(&self) -> Value {
        Value::Int(*self as i64)
    }
}
impl ToSqlValue for i64 {
    fn to_sql_value(&self) -> Value {
        Value::Int(*self)
    }
}
impl ToSqlValue for f32 {
    fn to_sql_value(&self) -> Value {
        Value::Real(*self as f64)
    }
}
impl ToSqlValue for f64 {
    fn to_sql_value(&self) -> Value {
        Value::Real(*self)
    }
}
impl ToSqlValue for bool {
    fn to_sql_value(&self) -> Value {
        Value::Bool(*self)
    }
}
impl ToSqlValue for String {
    fn to_sql_value(&self) -> Value {
        Value::Text(self.clone())
    }
}
impl ToSqlValue for &str {
    fn to_sql_value(&self) -> Value {
        Value::Text((*self).to_string())
    }
}
impl<T: ToSqlValue> ToSqlValue for Option<T> {
    fn to_sql_value(&self) -> Value {
        match self {
            Some(v) => v.to_sql_value(),
            None => Value::Null,
        }
    }
}

/// Marker for Rust types whose columns support ordering comparisons
/// (`<`, `>`, `<=`, `>=`, and ORDER BY). Numbers and strings qualify; booleans
/// do not.
pub trait Orderable {}

impl Orderable for i16 {}
impl Orderable for i32 {}
impl Orderable for i64 {}
impl Orderable for f32 {}
impl Orderable for f64 {}
impl Orderable for String {}

/// Marker for numeric Rust types, gating the arithmetic aggregates `SUM` and
/// `AVG` (which are meaningless on text or booleans).
pub trait Numeric {}

impl Numeric for i16 {}
impl Numeric for i32 {}
impl Numeric for i64 {}
impl Numeric for f32 {}
impl Numeric for f64 {}

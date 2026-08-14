//! Bound parameter values collected during rendering.

/// A concrete value bound as a query parameter.
///
/// This is a self-contained representation for the MVP so the core has no
/// database dependency and stays fast to compile and test. The future exec
/// layer will bridge these values to `sqlx` encoding.
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

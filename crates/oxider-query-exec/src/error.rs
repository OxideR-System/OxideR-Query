//! The execution layer's error type.
//!
//! Running a query can fail in two unrelated ways: the query cannot be
//! expressed in the target dialect, or the database rejected it. Keeping them
//! apart matters because only the first is a bug in the query itself, and it is
//! catchable in a test with no database running.

use oxider_query_core::RenderError;

/// Anything that can go wrong running a query.
#[derive(Debug)]
pub enum Error {
    /// The query could not be rendered for the connected database's dialect.
    Render(RenderError),
    /// The database rejected or failed to run the statement.
    Database(sqlx::Error),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Render(err) => write!(f, "could not render the query: {err}"),
            Error::Database(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Render(err) => Some(err),
            Error::Database(err) => Some(err),
        }
    }
}

impl From<RenderError> for Error {
    fn from(err: RenderError) -> Self {
        Error::Render(err)
    }
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        Error::Database(err)
    }
}

/// The result of an execution-layer call.
pub type Result<T> = core::result::Result<T, Error>;

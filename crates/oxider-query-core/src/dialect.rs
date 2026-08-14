//! SQL dialect abstraction.
//!
//! The MVP ships PostgreSQL only. MySQL and SQLite arrive in a later phase; the
//! trait boundary is deliberately extracted from a working Postgres impl rather
//! than designed up front, to avoid guessing the abstraction wrong.

/// Describes the syntactic quirks of a SQL dialect needed for rendering.
pub trait Dialect {
    /// Quote an identifier (table or column name), escaping as needed.
    fn quote_ident(&self, ident: &str) -> String;

    /// Render the positional placeholder for the Nth (1-based) bound parameter.
    fn placeholder(&self, index: usize) -> String;
}

/// PostgreSQL dialect: double-quoted identifiers, `$N` placeholders.
pub struct Postgres;

impl Dialect for Postgres {
    fn quote_ident(&self, ident: &str) -> String {
        // Postgres escapes an embedded double quote by doubling it.
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn placeholder(&self, index: usize) -> String {
        format!("${index}")
    }
}

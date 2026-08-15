//! PostgreSQL dialect.

use super::Dialect;

/// PostgreSQL: double-quoted identifiers, numbered `$N` placeholders.
#[derive(Debug, Default, Clone, Copy)]
pub struct Postgres;

impl Dialect for Postgres {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn quote_ident(&self, ident: &str) -> String {
        // Standard SQL: wrap in double quotes, doubling any embedded quote.
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn placeholder(&self, index: usize) -> String {
        format!("${index}")
    }
}

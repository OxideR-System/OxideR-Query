//! SQLite dialect.

use super::Dialect;

/// SQLite: double-quoted identifiers, positional `?` placeholders.
pub struct Sqlite;

impl Dialect for Sqlite {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn quote_ident(&self, ident: &str) -> String {
        // SQLite accepts standard double-quoted identifiers.
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn placeholder(&self, _index: usize) -> String {
        // SQLite uses unnumbered positional placeholders.
        "?".to_string()
    }
}

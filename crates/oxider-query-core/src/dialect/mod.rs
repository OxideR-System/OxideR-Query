//! SQL dialect abstraction.
//!
//! A dialect captures the syntactic differences between database engines that
//! affect how a query AST renders: how identifiers are quoted and how bound
//! parameters are referenced. The AST itself is dialect-agnostic, so supporting
//! a new engine means implementing exactly one trait.

mod mysql;
mod postgres;
mod sqlite;

pub use mysql::MySql;
pub use postgres::Postgres;
pub use sqlite::Sqlite;

/// Describes the syntactic quirks of a SQL dialect needed for rendering.
pub trait Dialect {
    /// A stable, human-readable name for the dialect (used in diagnostics).
    fn name(&self) -> &'static str;

    /// Quote an identifier (table or column name), escaping as needed.
    fn quote_ident(&self, ident: &str) -> String;

    /// Render the placeholder for the Nth (1-based) bound parameter.
    ///
    /// Numbered dialects (Postgres `$N`) use `index`; positional dialects
    /// (MySQL/SQLite `?`) ignore it.
    fn placeholder(&self, index: usize) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_escapes_embedded_double_quote() {
        assert_eq!(Postgres.quote_ident(r#"we"ird"#), r#""we""ird""#);
    }

    #[test]
    fn mysql_escapes_embedded_backtick() {
        assert_eq!(MySql.quote_ident("we`ird"), "`we``ird`");
    }

    #[test]
    fn sqlite_uses_standard_double_quotes() {
        assert_eq!(Sqlite.quote_ident("id"), r#""id""#);
    }

    #[test]
    fn placeholder_styles_differ() {
        assert_eq!(Postgres.placeholder(3), "$3");
        assert_eq!(MySql.placeholder(3), "?");
        assert_eq!(Sqlite.placeholder(3), "?");
    }
}

//! MySQL / MariaDB dialect.

use super::Dialect;

/// MySQL: backtick-quoted identifiers, positional `?` placeholders.
#[derive(Debug, Default, Clone, Copy)]
pub struct MySql;

impl Dialect for MySql {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn quote_ident(&self, ident: &str) -> String {
        // MySQL default: wrap in backticks, doubling any embedded backtick.
        format!("`{}`", ident.replace('`', "``"))
    }

    fn placeholder(&self, _index: usize) -> String {
        // MySQL uses unnumbered positional placeholders.
        "?".to_string()
    }
}

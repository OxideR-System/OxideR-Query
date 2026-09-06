//! Turning database names into Rust identifiers.
//!
//! A database is free to name a column `type`, `total-count` or `2fa enabled`.
//! Rust is not, so every generated name goes through here. Two rules matter:
//!
//! - the output must always parse as Rust, because a struct that does not
//!   compile is worse than no struct at all;
//! - the real database name must never be lost. When sanitising changes the
//!   name, the caller emits `#[oxider(column = "...")]` so the metamodel still
//!   queries the column that exists.

/// Rust keywords that cannot appear as a field name.
///
/// Strict and reserved keywords both, since a reserved word is a compile error
/// today in the editions this crate supports.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Keywords that are not valid raw identifiers either, so they can only be
/// escaped by changing the name.
const NEVER_RAW: &[&str] = &["crate", "self", "Self", "super"];

/// A field name, and the column name it has to be mapped back to.
pub(crate) struct FieldName {
    /// The identifier to write in the struct, already escaped if it needed it.
    pub ident: String,
    /// `Some(column)` when the identifier no longer spells the column name, so
    /// the caller must emit `#[oxider(column = "...")]`.
    pub rename: Option<String>,
}

/// Derive a usable field identifier from a column name.
pub(crate) fn field_name(column: &str) -> FieldName {
    let sanitized = sanitize(column);

    // A raw identifier keeps the name intact, and the derive macro strips the
    // `r#` back off, so no rename attribute is needed for a keyword.
    if KEYWORDS.contains(&sanitized.as_str()) && !NEVER_RAW.contains(&sanitized.as_str()) {
        return FieldName {
            ident: format!("r#{sanitized}"),
            rename: None,
        };
    }
    let ident = if NEVER_RAW.contains(&sanitized.as_str()) {
        format!("{sanitized}_")
    } else {
        sanitized
    };
    let rename = (ident != column).then(|| column.to_string());
    FieldName { ident, rename }
}

/// Derive a Rust type name from a table name, e.g. `order_items` ->
/// `OrderItems`, `user-sessions` -> `UserSessions`, `users` -> `Users`.
///
/// Any run of characters that cannot appear in an identifier separates words,
/// which is what makes `-`, `.` and spaces behave like `_`.
pub(crate) fn type_name(table: &str) -> String {
    let mut name: String = table
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();

    if name.is_empty() {
        // Every character was a separator. The table still exists, so give it a
        // name rather than emitting a struct with none.
        name.push_str("Table");
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if NEVER_RAW.contains(&name.as_str()) {
        name.push('_');
    }
    name
}

/// Replace everything that cannot appear in an identifier, and make sure the
/// result does not start with a digit.
fn sanitize(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{field_name, type_name};

    #[test]
    fn type_name_handles_snake_dash_and_plain() {
        assert_eq!(type_name("users"), "Users");
        assert_eq!(type_name("order_items"), "OrderItems");
        assert_eq!(type_name("user-sessions"), "UserSessions");
        assert_eq!(type_name("__weird__name_"), "WeirdName");
        assert_eq!(type_name("2fa_codes"), "_2faCodes");
        assert_eq!(type_name("---"), "Table");
        assert_eq!(type_name("self"), "Self_");
    }

    #[test]
    fn a_keyword_column_becomes_a_raw_identifier_with_no_rename() {
        let name = field_name("type");
        assert_eq!(name.ident, "r#type");
        assert_eq!(name.rename, None);
    }

    #[test]
    fn a_column_that_cannot_be_raw_is_renamed_and_mapped_back() {
        let name = field_name("crate");
        assert_eq!(name.ident, "crate_");
        assert_eq!(name.rename.as_deref(), Some("crate"));
    }

    #[test]
    fn a_column_with_illegal_characters_is_renamed_and_mapped_back() {
        let name = field_name("total-count");
        assert_eq!(name.ident, "total_count");
        assert_eq!(name.rename.as_deref(), Some("total-count"));

        let name = field_name("2fa enabled");
        assert_eq!(name.ident, "_2fa_enabled");
        assert_eq!(name.rename.as_deref(), Some("2fa enabled"));
    }

    #[test]
    fn an_ordinary_column_is_left_alone() {
        let name = field_name("email");
        assert_eq!(name.ident, "email");
        assert_eq!(name.rename, None);
    }
}

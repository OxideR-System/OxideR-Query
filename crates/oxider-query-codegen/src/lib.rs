//! `oxider-query-codegen`: generate OxideR-Query entity structs from an existing
//! database schema (the "introspection" metamodel path).
//!
//! Point it at a database, and it emits Rust source: one `#[derive(Entity)]`
//! struct per table, with fields typed from the columns and made `Option<_>`
//! when the column is nullable. Write that output to a file (checked in or built
//! by a build script) and you have a metamodel without hand-writing structs.
//!
//! Only SQLite introspection is implemented today (default feature `sqlite`);
//! Postgres and MySQL will follow behind their own features.
//!
//! ```no_run
//! # async fn demo() -> Result<(), sqlx::Error> {
//! use sqlx::SqlitePool;
//! let pool = SqlitePool::connect("sqlite::memory:").await?;
//! let source = oxider_query_codegen::generate_entities(&pool).await?;
//! std::fs::write("src/entities.rs", source).unwrap();
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "sqlite")]
pub use sqlite::generate_entities;

/// Convert a table name to a PascalCase Rust type name, e.g. `order_items` ->
/// `OrderItems`, `users` -> `Users`.
pub(crate) fn pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pascal_case;

    #[test]
    fn pascal_case_handles_snake_and_plain() {
        assert_eq!(pascal_case("users"), "Users");
        assert_eq!(pascal_case("order_items"), "OrderItems");
        assert_eq!(pascal_case("__weird__name_"), "WeirdName");
    }
}

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

mod identifier;

#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "sqlite")]
pub use sqlite::generate_entities;

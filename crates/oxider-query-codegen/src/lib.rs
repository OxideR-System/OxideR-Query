//! `oxider-query-codegen`: generate OxideR-Query entity structs from an existing
//! database schema (the "introspection" metamodel path).
//!
//! Point it at a database, and it emits Rust source: one `#[derive(Entity)]`
//! struct per table, with fields typed from the columns and made `Option<_>`
//! when the column is nullable. Write that output to a file (checked in or built
//! by a build script) and you have a metamodel without hand-writing structs.
//!
//! One module per engine, each behind its own feature: [`sqlite`] (on by
//! default) and [`postgres`]. They are separate modules rather than one
//! function because introspection is where two engines differ most - SQLite
//! infers a type from a free-text declaration, PostgreSQL reads a real one out
//! of the catalog, and only PostgreSQL has schemas and foreign keys to report.
//!
//! ```no_run
//! # async fn demo() -> Result<(), sqlx::Error> {
//! use sqlx::SqlitePool;
//! let pool = SqlitePool::connect("sqlite::memory:").await?;
//! let source = oxider_query_codegen::sqlite::generate_entities(&pool).await?;
//! std::fs::write("src/entities.rs", source).unwrap();
//! # Ok(())
//! # }
//! ```
//!
//! # Types that need a feature
//!
//! `chrono`, `rust_decimal`, `uuid` and `json` decide whether a date, numeric,
//! UUID or JSON column is generated as its real Rust type or left as `String`.
//! Turn on here whatever is on for `oxider-query`: naming a type the project
//! does not have produces a file that will not compile, which is worse than a
//! field someone has to refine by hand.
#![forbid(unsafe_code)]

mod identifier;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

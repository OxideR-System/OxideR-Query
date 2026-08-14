//! `oxider-query`: type-safe, multi-dialect SQL query builder for Rust.
//!
//! Inspired by Java's QueryDSL, but pushing type-safety further with Rust's type
//! system. Define entities as plain structs, derive [`Entity`], then build
//! queries whose column types are checked at compile time.
//!
//! ```
//! use oxider_query::prelude::*;
//!
//! #[derive(Entity)]
//! #[oxider(table = "users")]
//! struct User {
//!     id: i64,
//!     name: String,
//!     email: Option<String>,
//! }
//!
//! let u = User::table();
//! let rendered = Query::select()
//!     .from(u)
//!     .select((u.id, u.name))
//!     .filter(u.name.eq("Alice"))
//!     .render(&Postgres);
//!
//! assert_eq!(
//!     rendered.sql,
//!     r#"SELECT "users"."id", "users"."name" FROM "users" WHERE ("users"."name" = $1)"#
//! );
//! ```
//!
//! This facade re-exports the core crate and the derive macro, so downstream
//! crates only depend on `oxider-query`.

pub use oxider_query_core::*;
pub use oxider_query_macros::Entity;

/// Common imports for building queries.
pub mod prelude {
    pub use oxider_query_core::{Dialect, Expression, ExpressionMethods, Postgres, Query, Table};
    pub use oxider_query_macros::Entity;
}

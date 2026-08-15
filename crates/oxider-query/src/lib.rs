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
//!     age: i32,
//!     email: Option<String>,
//! }
//!
//! let rendered = User::query()
//!     .select((User::id, User::name))
//!     .filter(User::name.contains("nguyen").and(User::age.ge(18)))
//!     .order_by(User::id.desc())
//!     .limit(20)
//!     .render(&Postgres);
//! # let _ = rendered;
//! ```
//!
//! Comparing a column against the wrong type is a compile error:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! User::name.eq(123); // error: the trait bound `i64: Into<String>` is not satisfied
//! ```
//!
//! Referencing a column of an entity you forgot to join is also a compile error;
//! the referenced table must be in scope (via FROM or a JOIN):
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! # #[derive(Entity)] #[oxider(table = "departments")]
//! # struct Department { id: i64, name: String }
//! // Department was never joined -> `Nil: Contains<Department>` is not satisfied.
//! User::query().filter(Department::name.eq("AI"));
//! ```
//!
//! This facade re-exports the core crate and the derive macro, so downstream
//! crates only depend on `oxider-query`.

pub use oxider_query_core::*;
pub use oxider_query_macros::Entity;

/// Common imports for building queries.
pub mod prelude {
    pub use oxider_query_core::{
        Column, Dialect, Entity, MySql, OrderTerm, Postgres, Predicate, Sqlite, ToSqlValue,
    };
    pub use oxider_query_macros::Entity;
}

//! `oxider-query-exec`: async execution layer bridging OxideR-Query queries to
//! [`sqlx`].
//!
//! The functions here take any [`Renderable`](oxider_query_core::Renderable)
//! query built with `oxider-query-core`, render it with the connected database's
//! dialect, bind its parameters and run the statement on a sqlx connection or
//! pool, mapping result rows into any type implementing sqlx's `FromRow`.
//!
//! Because the dialect is chosen here, from the database, call sites never spell
//! `.render(&Sqlite)`: they pass the query directly. Queries stay
//! dialect-agnostic, so pointing at a different database does not touch query
//! code.
//!
//! Backends are feature-gated. Only SQLite is implemented today (default
//! feature `sqlite`); Postgres and MySQL execution will follow the same shape
//! behind their own features. The query builder is already multi-dialect, so the
//! remaining work per backend is just parameter binding and row typing.
//!
//! ```no_run
//! # async fn demo() -> Result<(), sqlx::Error> {
//! use oxider_query::prelude::*;
//! use sqlx::SqlitePool;
//!
//! #[derive(Entity, sqlx::FromRow)]
//! #[oxider(table = "users")]
//! struct User { id: i64, name: String }
//!
//! let pool = SqlitePool::connect("sqlite::memory:").await?;
//! // No `.render(&Sqlite)` here - the pool's backend picks the dialect.
//! let users: Vec<User> =
//!     oxider_query_exec::fetch_all(&pool, User::query().filter(User::id.gt(0))).await?;
//! # let _ = users;
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "sqlite")]
pub use sqlite::{execute, fetch_all, fetch_one, fetch_optional};

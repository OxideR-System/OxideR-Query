//! `oxider-query-exec`: async execution layer bridging OxideR-Query's rendered
//! SQL to [`sqlx`].
//!
//! The query builder in `oxider-query-core` produces a
//! [`Rendered`](oxider_query_core::Rendered) `(sql, params)` pair. This crate
//! binds those parameters and runs the statement on a
//! sqlx connection or pool, mapping result rows into any type implementing
//! sqlx's `FromRow`.
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
//! let rendered = User::query().filter(User::id.gt(0)).render(&Sqlite);
//! let users: Vec<User> = oxider_query_exec::fetch_all(&pool, &rendered).await?;
//! # let _ = users;
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "sqlite")]
pub use sqlite::{execute, fetch_all, fetch_one, fetch_optional};

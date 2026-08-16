//! `oxider-query-exec`: async execution layer bridging OxideR-Query queries to
//! [`sqlx`].
//!
//! The entry point is [`Db`], one handle that wraps a sqlx pool for any backend.
//! Its methods take any [`Renderable`](oxider_query_core::Renderable) query,
//! render it with the backend's dialect, bind the parameters and run the
//! statement, mapping result rows into any type implementing sqlx's `FromRow`.
//!
//! Because [`Db`] chooses the dialect from its backend, call sites never spell
//! `.render(&Sqlite)`: they pass the query directly. Queries stay
//! dialect-agnostic, so pointing at a different database is a one-line change of
//! the handle's type and touches no query code.
//!
//! [`Db::begin`] starts a transaction; the returned [`Tx`] handle exposes the
//! same query methods and is finished with [`Tx::commit`] or [`Tx::rollback`].
//!
//! Backends are feature-gated via the [`Backend`] trait. Only SQLite is
//! implemented today (default feature `sqlite`, handle alias [`SqliteDb`]);
//! Postgres and MySQL slot in by implementing [`Backend`] for their sqlx
//! `Database`, with no change to [`Db`] or [`Tx`].
//!
//! ```no_run
//! # async fn demo() -> Result<(), sqlx::Error> {
//! use oxider_query::prelude::*;
//! use oxider_query_exec::SqliteDb;
//!
//! #[derive(Entity, sqlx::FromRow)]
//! #[oxider(table = "users")]
//! struct User { id: i64, name: String }
//!
//! let db = SqliteDb::connect("sqlite::memory:").await?;
//! // No `.render(&Sqlite)` here - the handle's backend picks the dialect.
//! let users: Vec<User> = db.fetch_all(User::query().filter(User::id.gt(0))).await?;
//! # let _ = users;
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

use oxider_query_core::Dialect;
use sqlx::Database;

mod db;
mod ops;
mod tx;

#[cfg(feature = "sqlite")]
mod sqlite;

pub use db::Db;
pub use tx::Tx;

/// A database backend: the SQL dialect to render for, plus how to bind rendered
/// parameters onto that backend's sqlx queries.
///
/// Implemented per sqlx `Database` behind a crate feature. All the encoding
/// concerns live inside the concrete impl, so [`Db`] and [`Tx`] stay
/// backend-agnostic.
pub trait Backend: Database {
    /// The dialect [`Db`] renders queries with for this backend.
    type Dialect: Dialect + Default;

    /// Bind rendered parameters onto a plain query, in placeholder order.
    fn bind<'q>(
        query: sqlx::query::Query<'q, Self, <Self as Database>::Arguments<'q>>,
        params: &'q [oxider_query_core::Value],
    ) -> sqlx::query::Query<'q, Self, <Self as Database>::Arguments<'q>>;

    /// Bind rendered parameters onto a typed (`FromRow`) query, in order.
    fn bind_as<'q, O>(
        query: sqlx::query::QueryAs<'q, Self, O, <Self as Database>::Arguments<'q>>,
        params: &'q [oxider_query_core::Value],
    ) -> sqlx::query::QueryAs<'q, Self, O, <Self as Database>::Arguments<'q>>;

    /// Extract the affected-row count from a statement result (no unified sqlx
    /// trait method exposes it, so each backend forwards to its own type).
    fn rows_affected(result: <Self as Database>::QueryResult) -> u64;
}

/// A [`Db`] handle for SQLite.
#[cfg(feature = "sqlite")]
pub type SqliteDb = Db<sqlx::Sqlite>;

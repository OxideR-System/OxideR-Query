//! `oxider-query-exec`: async execution layer bridging OxideR-Query queries to
//! [`sqlx`].
//!
//! The entry point is [`Db`], one handle that wraps a sqlx pool for any backend.
//! Its methods take any [`Renderable`] query,
//! render it with the backend's dialect, bind the parameters and run the
//! statement, mapping result rows into any type implementing sqlx's `FromRow`.
//!
//! Because [`Db`] chooses the dialect from its backend, call sites never spell
//! `.render(&Sqlite)`: they pass the query directly. Queries stay
//! dialect-agnostic, so pointing at a different database is a one-line change of
//! the handle's type and touches no query code.
//!
//! Backends are feature-gated via the [`Backend`] trait. Only SQLite is
//! implemented today (default feature `sqlite`, handle alias [`SqliteDb`]);
//! Postgres and MySQL slot in by implementing [`Backend`] for their sqlx
//! `Database`, with no change to [`Db`].
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

use oxider_query_core::{Dialect, Renderable};
use sqlx::{Database, FromRow, Pool};

#[cfg(feature = "sqlite")]
mod sqlite;

/// A database backend: the SQL dialect to render for, plus how to bind rendered
/// parameters onto that backend's sqlx queries.
///
/// Implemented per sqlx `Database` behind a crate feature. All the encoding
/// concerns live inside the concrete impl, so [`Db`] stays backend-agnostic.
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

/// A database handle: a sqlx pool paired with its backend's dialect.
///
/// Construct one with [`Db::connect`] or [`Db::new`], then run queries with
/// [`execute`](Db::execute), [`fetch_all`](Db::fetch_all),
/// [`fetch_one`](Db::fetch_one) or [`fetch_optional`](Db::fetch_optional). The
/// same handle runs SELECTs and mutations; the dialect is never named at the
/// call site.
pub struct Db<DB: Backend> {
    pool: Pool<DB>,
}

impl<DB: Backend> Db<DB>
where
    // sqlx needs the backend's arguments to be convertible for `execute`/`fetch`,
    // and a pool reference to be an executor. Both hold for every sqlx database;
    // stating them once here keeps the method signatures clean.
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
{
    /// Wrap an existing sqlx pool.
    pub fn new(pool: Pool<DB>) -> Self {
        Self { pool }
    }

    /// Open a pool from a connection URL.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        Ok(Self {
            pool: Pool::connect(url).await?,
        })
    }

    /// Borrow the underlying sqlx pool (for transactions or raw sqlx access).
    pub fn pool(&self) -> &Pool<DB> {
        &self.pool
    }

    /// Run a statement (typically INSERT/UPDATE/DELETE) and return the number of
    /// affected rows.
    pub async fn execute<Q>(&self, query: Q) -> Result<u64, sqlx::Error>
    where
        Q: Renderable,
    {
        let rendered = query.render_with(&DB::Dialect::default());
        let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);
        Ok(DB::rows_affected(bound.execute(&self.pool).await?))
    }

    /// Run a query and collect every row into `O`.
    pub async fn fetch_all<O, Q>(&self, query: Q) -> Result<Vec<O>, sqlx::Error>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        let rendered = query.render_with(&DB::Dialect::default());
        let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);
        bound.fetch_all(&self.pool).await
    }

    /// Run a query expected to return exactly one row.
    pub async fn fetch_one<O, Q>(&self, query: Q) -> Result<O, sqlx::Error>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        let rendered = query.render_with(&DB::Dialect::default());
        let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);
        bound.fetch_one(&self.pool).await
    }

    /// Run a query that may return zero or one row.
    pub async fn fetch_optional<O, Q>(&self, query: Q) -> Result<Option<O>, sqlx::Error>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        let rendered = query.render_with(&DB::Dialect::default());
        let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);
        bound.fetch_optional(&self.pool).await
    }
}

/// A [`Db`] handle for SQLite.
#[cfg(feature = "sqlite")]
pub type SqliteDb = Db<sqlx::Sqlite>;

//! Shared execution helpers, generic over any sqlx executor.
//!
//! Both [`Db`](crate::Db) (running on a pool) and [`Tx`](crate::Tx) (running on
//! a transaction) render, bind and run in the same way; only the executor
//! differs. These helpers hold that logic once so the two handles stay thin.

use crate::{Backend, Result};
use oxider_query_core::Renderable;
use sqlx::{Database, Executor, FromRow};

/// Render, bind and run a statement, returning the affected-row count.
pub(crate) async fn execute<'e, DB, E, Q>(executor: E, query: Q) -> Result<u64>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);
    Ok(DB::rows_affected(bound.execute(executor).await?))
}

/// Render, bind and run a query, collecting every row into `O`.
pub(crate) async fn fetch_all<'e, DB, E, O, Q>(executor: E, query: Q) -> Result<Vec<O>>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);
    Ok(bound.fetch_all(executor).await?)
}

/// Render, bind and run a query expected to return exactly one row.
pub(crate) async fn fetch_one<'e, DB, E, O, Q>(executor: E, query: Q) -> Result<O>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);
    Ok(bound.fetch_one(executor).await?)
}

/// Render, bind and run a query that may return zero or one row.
pub(crate) async fn fetch_optional<'e, DB, E, O, Q>(executor: E, query: Q) -> Result<Option<O>>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);
    Ok(bound.fetch_optional(executor).await?)
}

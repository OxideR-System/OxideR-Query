//! Shared execution helpers, generic over any sqlx executor.
//!
//! Both [`Db`](crate::Db) (running on a pool) and [`Tx`](crate::Tx) (running on
//! a transaction) render, bind and run in the same way; only the executor
//! differs. These helpers hold that logic once so the two handles stay thin.

use crate::{Backend, Projection, Result};
use futures_core::Stream;
use oxider_query_core::Renderable;
use sqlx::{Database, Executor, FromRow};
use std::future::poll_fn;

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

/// Render, bind and run a query, handing each row to `f` as it arrives.
///
/// The point is what it does *not* do: no `Vec` is built, so a query returning
/// more rows than fit in memory is a loop rather than an allocation failure.
/// Returns how many rows went past.
///
/// `f` returning an error stops the walk and that error is the result, so a
/// caller can give up early without draining the rest.
///
/// This is a fold rather than a `Stream` on purpose. A `Stream` would have to
/// own the rendered SQL and borrow from it at the same time, which needs either
/// a self-referential type or a generator macro from another crate; neither is
/// worth it when the reason to stream is to avoid holding the rows. Anyone who
/// needs a real `Stream` to compose with can drive sqlx over
/// [`Db::pool`](crate::Db::pool).
pub(crate) async fn for_each_row<'e, DB, E, O, Q, F>(executor: E, query: Q, mut f: F) -> Result<u64>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
    Q: Renderable,
    F: FnMut(O) -> Result<()>,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind_as(sqlx::query_as::<DB, O>(&rendered.sql), &rendered.params);

    // `fetch` yields a `BoxStream`, and polling it by hand is what keeps
    // `futures-util` out of the dependency list for one call to `next`.
    let mut rows = bound.fetch(executor);
    let mut seen = 0;
    while let Some(row) = poll_fn(|cx| Stream::poll_next(rows.as_mut(), cx)).await {
        f(row?)?;
        seen += 1;
    }
    Ok(seen)
}

/// The same, reading each row by column position rather than by name.
pub(crate) async fn for_each_row_projected<'e, DB, E, O, Q, F>(
    executor: E,
    query: Q,
    mut f: F,
) -> Result<u64>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> Projection<'r, DB::Row>,
    Q: Renderable,
    F: FnMut(O) -> Result<()>,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);

    let mut rows = bound.fetch(executor);
    let mut seen = 0;
    while let Some(row) = poll_fn(|cx| Stream::poll_next(rows.as_mut(), cx)).await {
        f(O::from_row_at(&row?, 0)?)?;
        seen += 1;
    }
    Ok(seen)
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

/// Render, bind and run a query, reading each row by column position.
///
/// The rows come back raw and are decoded here rather than through sqlx's
/// `query_as`, because that path goes via `FromRow`, which matches by name.
pub(crate) async fn fetch_all_projected<'e, DB, E, O, Q>(executor: E, query: Q) -> Result<Vec<O>>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> Projection<'r, DB::Row>,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);
    let rows = bound.fetch_all(executor).await?;
    rows.iter()
        .map(|row| O::from_row_at(row, 0).map_err(Into::into))
        .collect()
}

/// Render, bind and run a query expected to return exactly one row, read by
/// column position.
pub(crate) async fn fetch_one_projected<'e, DB, E, O, Q>(executor: E, query: Q) -> Result<O>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> Projection<'r, DB::Row>,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);
    let row = bound.fetch_one(executor).await?;
    Ok(O::from_row_at(&row, 0)?)
}

/// Render, bind and run a query that may return zero or one row, read by column
/// position.
pub(crate) async fn fetch_optional_projected<'e, DB, E, O, Q>(
    executor: E,
    query: Q,
) -> Result<Option<O>>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    O: for<'r> Projection<'r, DB::Row>,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);
    match bound.fetch_optional(executor).await? {
        Some(row) => Ok(Some(O::from_row_at(&row, 0)?)),
        None => Ok(None),
    }
}

/// Render, bind and run a counting query, reading its single value.
///
/// The count comes back as a signed integer because that is what every engine
/// gives; it cannot be negative, so the conversion is a clamp rather than a
/// decision.
pub(crate) async fn fetch_count<'e, DB, E, Q>(executor: E, query: Q) -> Result<u64>
where
    DB: Backend,
    E: Executor<'e, Database = DB>,
    Q: Renderable,
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
    usize: sqlx::ColumnIndex<DB::Row>,
{
    let rendered = query.render_with(&DB::Dialect::default())?;
    let bound = DB::bind(sqlx::query::<DB>(&rendered.sql), &rendered.params);
    let row = bound.fetch_one(executor).await?;
    let count: i64 = sqlx::Row::try_get(&row, 0)?;
    Ok(count.max(0) as u64)
}

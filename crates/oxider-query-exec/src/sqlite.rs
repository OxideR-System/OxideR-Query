//! SQLite execution backend.
//!
//! Takes any [`Renderable`] query, renders it with the SQLite dialect, binds its
//! parameters onto a sqlx SQLite query and runs it. Because the dialect is chosen
//! here from the connected database, call sites never spell `.render(&Sqlite)`.
//! `fetch_*` map rows into a `FromRow` type; `execute` runs a statement and
//! returns the affected row count.

use oxider_query_core::{Renderable, Sqlite as SqliteDialect, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::{Executor, FromRow, Sqlite};

/// Bind a rendered statement's parameters onto a sqlx query, in order.
///
/// SQLite has no native boolean; sqlx encodes `bool` as an integer, matching how
/// values round-trip. `NULL` binds as a typed `None` so sqlx sends SQL NULL.
macro_rules! bind_params {
    ($query:expr, $params:expr) => {{
        let mut query = $query;
        for value in $params {
            query = match value {
                Value::Bool(b) => query.bind(*b),
                Value::Int(i) => query.bind(*i),
                Value::Real(r) => query.bind(*r),
                Value::Text(s) => query.bind(s.clone()),
                Value::Null => query.bind(Option::<i64>::None),
            };
        }
        query
    }};
}

/// Run a statement (typically INSERT/UPDATE/DELETE) and return the number of
/// affected rows.
pub async fn execute<'e, E, Q>(executor: E, query: Q) -> Result<u64, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
    Q: Renderable,
{
    let rendered = query.render_with(&SqliteDialect);
    let bound = bind_params!(sqlx::query::<Sqlite>(&rendered.sql), &rendered.params);
    Ok(bound.execute(executor).await?.rows_affected())
}

/// Run a query and collect every row into `O`.
pub async fn fetch_all<'e, E, O, Q>(executor: E, query: Q) -> Result<Vec<O>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
    O: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    Q: Renderable,
{
    let rendered = query.render_with(&SqliteDialect);
    let bound = bind_params!(sqlx::query_as::<Sqlite, O>(&rendered.sql), &rendered.params);
    bound.fetch_all(executor).await
}

/// Run a query expected to return exactly one row.
pub async fn fetch_one<'e, E, O, Q>(executor: E, query: Q) -> Result<O, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
    O: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    Q: Renderable,
{
    let rendered = query.render_with(&SqliteDialect);
    let bound = bind_params!(sqlx::query_as::<Sqlite, O>(&rendered.sql), &rendered.params);
    bound.fetch_one(executor).await
}

/// Run a query that may return zero or one row.
pub async fn fetch_optional<'e, E, O, Q>(executor: E, query: Q) -> Result<Option<O>, sqlx::Error>
where
    E: Executor<'e, Database = Sqlite>,
    O: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    Q: Renderable,
{
    let rendered = query.render_with(&SqliteDialect);
    let bound = bind_params!(sqlx::query_as::<Sqlite, O>(&rendered.sql), &rendered.params);
    bound.fetch_optional(executor).await
}

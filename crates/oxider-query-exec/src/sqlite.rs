//! SQLite backend: renders with the SQLite dialect and binds parameters onto
//! sqlx SQLite queries. All the encoding lives here, where the database type is
//! concrete, so [`Db`](crate::Db) stays backend-agnostic.

use crate::Backend;
use oxider_query_core::Sqlite as SqliteDialect;
use oxider_query_core::Value;
use sqlx::query::{Query, QueryAs};
use sqlx::sqlite::{SqliteArguments, SqliteQueryResult};
use sqlx::Sqlite;

/// Bind a rendered statement's parameters onto a sqlx query, in order.
///
/// SQLite has no native boolean; sqlx encodes `bool` as an integer, matching how
/// values round-trip. Temporal values bind as text in the ISO-8601 form SQLite's
/// own date functions expect. `NULL` binds as a typed `None` so sqlx sends SQL
/// NULL.
///
/// Text and blobs bind by reference. The parameter slice outlives the query -
/// that is what the `'q` lifetime on `params` says - so copying a large payload
/// here would be a second copy of something the renderer already cloned once.
macro_rules! bind_params {
    ($query:expr, $params:expr) => {{
        let mut query = $query;
        for value in $params {
            query = match value {
                Value::Bool(b) => query.bind(*b),
                Value::Int(i) => query.bind(*i),
                Value::Real(r) => query.bind(*r),
                Value::Text(s) => query.bind(s.as_str()),
                Value::Bytes(b) => query.bind(b.as_slice()),
                Value::Date(s) | Value::Time(s) | Value::DateTime(s) => query.bind(s.as_str()),
                Value::Null => query.bind(Option::<i64>::None),
            };
        }
        query
    }};
}

impl Backend for Sqlite {
    type Dialect = SqliteDialect;

    fn bind<'q>(
        query: Query<'q, Sqlite, SqliteArguments<'q>>,
        params: &'q [Value],
    ) -> Query<'q, Sqlite, SqliteArguments<'q>> {
        bind_params!(query, params)
    }

    fn bind_as<'q, O>(
        query: QueryAs<'q, Sqlite, O, SqliteArguments<'q>>,
        params: &'q [Value],
    ) -> QueryAs<'q, Sqlite, O, SqliteArguments<'q>> {
        bind_params!(query, params)
    }

    fn rows_affected(result: SqliteQueryResult) -> u64 {
        result.rows_affected()
    }
}

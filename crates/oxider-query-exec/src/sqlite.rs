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

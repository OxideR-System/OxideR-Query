//! PostgreSQL backend: renders with the Postgres dialect and binds parameters
//! onto sqlx Postgres queries.
//!
//! Postgres is the strict one. SQLite types a column by the value it is handed
//! and MySQL coerces freely, so both accept a date sent as text. Postgres
//! carries a type OID per bound parameter, sqlx sends `text` for a Rust string,
//! and the server refuses it where a `date` is expected:
//!
//! ```text
//! ERROR 42804: column "on_day" is of type date but expression is of type text
//! ```
//!
//! [`Value`] stores temporals as text, because the AST must not depend on a
//! date library. So this backend reads them back into `chrono` types before
//! binding, using the same format constants the core crate wrote them with.

use crate::Backend;
use oxider_query_core::formats::{DATE, DATE_TIME, DATE_TIME_UTC, TIME};
use oxider_query_core::Postgres as PostgresDialect;
use oxider_query_core::Value;
use sqlx::postgres::{PgArguments, PgQueryResult};
use sqlx::query::{Query, QueryAs};
use sqlx::Postgres;

/// A timestamp, as whichever of the two forms [`Value::DateTime`] can hold.
///
/// `DateTime<Utc>` is written with an explicit offset so a zoned column does not
/// reinterpret it, while `NaiveDateTime` has none. Both arrive here as text, so
/// the offset form is tried first: it is the more specific of the two, and a
/// naive string cannot parse as it.
enum Stamp {
    Zoned(chrono::DateTime<chrono::Utc>),
    Naive(chrono::NaiveDateTime),
}

fn stamp(text: &str) -> Option<Stamp> {
    if let Ok(zoned) = chrono::DateTime::parse_from_str(text, DATE_TIME_UTC) {
        return Some(Stamp::Zoned(zoned.into()));
    }
    chrono::NaiveDateTime::parse_from_str(text, DATE_TIME)
        .ok()
        .map(Stamp::Naive)
}

/// Bind a rendered statement's parameters onto a sqlx query, in order.
///
/// Text and blobs bind by reference: the parameter slice outlives the query, so
/// copying a large payload here would be a second copy of something the
/// renderer already cloned once.
///
/// A temporal that does not parse binds as text and lets Postgres decide. That
/// can only happen for a `Value` this crate did not produce, since everything
/// going through `ToSqlValue` is written with the very constants read back
/// here, and it yields the same error a hand-written query would.
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
                Value::Date(s) => match chrono::NaiveDate::parse_from_str(s, DATE) {
                    Ok(date) => query.bind(date),
                    Err(_) => query.bind(s.as_str()),
                },
                Value::Time(s) => match chrono::NaiveTime::parse_from_str(s, TIME) {
                    Ok(time) => query.bind(time),
                    Err(_) => query.bind(s.as_str()),
                },
                Value::DateTime(s) => match stamp(s) {
                    Some(Stamp::Zoned(instant)) => query.bind(instant),
                    Some(Stamp::Naive(naive)) => query.bind(naive),
                    None => query.bind(s.as_str()),
                },
                Value::Null => query.bind(Option::<i64>::None),
            };
        }
        query
    }};
}

impl Backend for Postgres {
    type Dialect = PostgresDialect;

    fn bind<'q>(
        query: Query<'q, Postgres, PgArguments>,
        params: &'q [Value],
    ) -> Query<'q, Postgres, PgArguments> {
        bind_params!(query, params)
    }

    fn bind_as<'q, O>(
        query: QueryAs<'q, Postgres, O, PgArguments>,
        params: &'q [Value],
    ) -> QueryAs<'q, Postgres, O, PgArguments> {
        bind_params!(query, params)
    }

    fn rows_affected(result: PgQueryResult) -> u64 {
        result.rows_affected()
    }
}

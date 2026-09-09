//! MySQL backend: renders with the MySQL dialect and binds parameters onto sqlx
//! MySQL queries.
//!
//! MySQL sits between the other two backends. It coerces freely enough to take a
//! date sent as text, the way SQLite does, but its wire protocol has a distinct
//! binary encoding for temporals, and a string carrying a UTC offset -
//! `2024-03-15 09:30:00+00:00` - is not a datetime literal it accepts. So this
//! backend reads temporals back into `chrono` types before binding, exactly as
//! the Postgres one does, using the same format constants the core crate wrote
//! them with.
//!
//! # Instants and the session time zone
//!
//! MySQL has no zoned datetime type. `DATETIME` is a wall clock with no zone at
//! all, and `TIMESTAMP` is stored as UTC but converted on the way in and out
//! using the connection's `time_zone`.
//!
//! A [`Value::DateTime`] holding an instant is therefore bound as **its UTC wall
//! clock**, with the offset dropped, because there is nowhere to put it. Written
//! to a `DATETIME` that is exactly right. Written to a `TIMESTAMP` it is right
//! only when the connection's `time_zone` is UTC, because otherwise the server
//! reads those digits as local time and shifts them.
//!
//! This crate does not set `time_zone` for you - the pool is built by sqlx and
//! the handle does not intercept connections. Either use `DATETIME` for
//! instants, or set the session zone yourself:
//!
//! ```text
//! SET time_zone = '+00:00'
//! ```

use crate::Backend;
use oxider_query_core::formats::{DATE, DATE_TIME, DATE_TIME_UTC, TIME};
use oxider_query_core::MySql as MySqlDialect;
use oxider_query_core::Value;
use sqlx::mysql::{MySqlArguments, MySqlQueryResult};
use sqlx::query::{Query, QueryAs};
use sqlx::MySql;

/// Parse a [`Value::DateTime`] into the naive datetime to bind.
///
/// Both forms the variant can hold end up as a naive value here. The offset form
/// is tried first because it is the more specific of the two and a naive string
/// cannot parse as it; its offset is then dropped by taking the UTC wall clock,
/// which is the only shape MySQL has a column type for. See the module docs for
/// what that means for `TIMESTAMP`.
fn naive_stamp(text: &str) -> Option<chrono::NaiveDateTime> {
    if let Ok(zoned) = chrono::DateTime::parse_from_str(text, DATE_TIME_UTC) {
        return Some(zoned.naive_utc());
    }
    chrono::NaiveDateTime::parse_from_str(text, DATE_TIME).ok()
}

/// Bind a rendered statement's parameters onto a sqlx query, in order.
///
/// Text and blobs bind by reference: the parameter slice outlives the query, so
/// copying a large payload here would be a second copy of something the
/// renderer already cloned once.
///
/// A temporal that does not parse binds as text and lets MySQL decide. That can
/// only happen for a `Value` this crate did not produce, since everything going
/// through `ToSqlValue` is written with the very constants read back here.
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
                Value::DateTime(s) => match naive_stamp(s) {
                    Some(stamp) => query.bind(stamp),
                    None => query.bind(s.as_str()),
                },
                Value::Null => query.bind(Option::<i64>::None),
            };
        }
        query
    }};
}

impl Backend for MySql {
    type Dialect = MySqlDialect;

    fn bind<'q>(
        query: Query<'q, MySql, MySqlArguments>,
        params: &'q [Value],
    ) -> Query<'q, MySql, MySqlArguments> {
        bind_params!(query, params)
    }

    fn bind_as<'q, O>(
        query: QueryAs<'q, MySql, O, MySqlArguments>,
        params: &'q [Value],
    ) -> QueryAs<'q, MySql, O, MySqlArguments> {
        bind_params!(query, params)
    }

    fn rows_affected(result: MySqlQueryResult) -> u64 {
        result.rows_affected()
    }
}

//! The schema and assertions every scenario shares.
//!
//! Tests here are scenarios rather than unit tests: each one builds a query a
//! real application would build and pins the complete SQL and the complete
//! parameter list, for every dialect that can express it. Pinning the whole
//! string is deliberate - a partial `contains` assertion would have missed the
//! precedence, quoting and placeholder-numbering bugs these tests exist to
//! catch.

#![allow(dead_code)]

use oxider_query::prelude::*;
use oxider_query::{Dialect, Renderable, Rendered, Value};

pub use chrono::{NaiveDate, NaiveDateTime};

/// A person who signs in, writes posts and places orders.
#[derive(Entity)]
#[oxider(table = "users")]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub age: i32,
    pub active: bool,
    pub manager_id: Option<i64>,
    pub department_id: Option<i64>,
    pub created_at: NaiveDateTime,
}

/// The organisational unit a user belongs to.
#[derive(Entity)]
#[oxider(table = "departments")]
pub struct Department {
    pub id: i64,
    pub name: String,
    pub budget: f64,
}

/// An article written by a user.
#[derive(Entity)]
#[oxider(table = "posts")]
pub struct Post {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub body: Option<String>,
    pub views: i64,
    pub published_at: Option<NaiveDateTime>,
}

/// A purchase.
#[derive(Entity)]
#[oxider(table = "orders")]
pub struct Order {
    pub id: i64,
    pub user_id: i64,
    pub total: f64,
    pub status: String,
    pub placed_on: NaiveDate,
}

/// One line of an order.
#[derive(Entity)]
#[oxider(table = "order_items")]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub product: String,
    pub quantity: i32,
    pub price: f64,
}

/// An entity in a named schema, for qualified-name scenarios.
#[derive(Entity)]
#[oxider(table = "audit_log", schema = "ops")]
pub struct AuditLog {
    pub id: i64,
    #[oxider(column = "actor")]
    pub actor_name: String,
    pub action: String,
}

/// Render a query, failing the test with the dialect's own message if it
/// cannot be expressed.
#[track_caller]
pub fn render<Q: Renderable>(query: Q, dialect: &dyn Dialect) -> Rendered {
    match query.render_with(dialect) {
        Ok(rendered) => rendered,
        Err(err) => panic!("{} could not render the query: {err}", dialect.name()),
    }
}

/// Assert the full SQL text and the full parameter list for one dialect.
#[track_caller]
pub fn assert_sql<Q: Renderable>(
    query: Q,
    dialect: &dyn Dialect,
    expected_sql: &str,
    expected_params: &[Value],
) {
    let rendered = render(query, dialect);
    assert_eq!(
        rendered.sql,
        expected_sql,
        "\n{} produced different SQL\n  actual:   {}\n  expected: {}\n",
        dialect.name(),
        rendered.sql,
        expected_sql
    );
    assert_eq!(
        rendered.params.as_slice(),
        expected_params,
        "\n{} bound different parameters",
        dialect.name()
    );
}

/// Assert the SQL text for one dialect, for queries that bind nothing.
#[track_caller]
pub fn assert_sql_only<Q: Renderable>(query: Q, dialect: &dyn Dialect, expected_sql: &str) {
    assert_sql(query, dialect, expected_sql, &[]);
}

/// Assert the dialect refuses the query, with a message mentioning `needle`.
///
/// Refusing to render is the designed behaviour for a construct an engine
/// cannot run: a wrong-but-plausible statement failing at the database is
/// harder to diagnose than a rejection here.
#[track_caller]
pub fn assert_rejected<Q: Renderable>(query: Q, dialect: &dyn Dialect, needle: &str) {
    match query.render_with(dialect) {
        Ok(rendered) => panic!(
            "{} was expected to reject the query but rendered: {}",
            dialect.name(),
            rendered.sql
        ),
        Err(err) => {
            let message = err.to_string();
            assert!(
                message.contains(needle),
                "{} rejected the query with `{message}`, which does not mention `{needle}`",
                dialect.name()
            );
        }
    }
}

/// A bound text parameter.
pub fn text(value: &str) -> Value {
    Value::Text(value.to_string())
}

/// A bound integer parameter.
pub fn int(value: i64) -> Value {
    Value::Int(value)
}

/// A bound floating-point parameter.
pub fn real(value: f64) -> Value {
    Value::Real(value)
}

/// A bound boolean parameter.
pub fn flag(value: bool) -> Value {
    Value::Bool(value)
}

/// A bound date parameter, in the form the value layer produces.
pub fn date(value: &str) -> Value {
    Value::Date(value.to_string())
}

/// A bound timestamp parameter, in the form the value layer produces.
pub fn timestamp(value: &str) -> Value {
    Value::DateTime(value.to_string())
}

/// A fixed date, so scenarios do not depend on the clock.
pub fn a_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, 3, 15).expect("valid date")
}

/// A fixed timestamp, so scenarios do not depend on the clock.
pub fn a_timestamp() -> NaiveDateTime {
    a_date().and_hms_opt(9, 30, 0).expect("valid time")
}

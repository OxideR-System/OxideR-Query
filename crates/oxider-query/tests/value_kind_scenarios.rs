//! Scenarios for the value kinds that cross the AST as text: exact decimals,
//! UUIDs and JSON.
//!
//! What matters here is that each one binds as its own `Value` kind rather than
//! being flattened into `Text` or `Real`, because that kind is what tells a
//! backend which of its own types to hand the driver. The digits of a decimal
//! surviving is the whole point of using one.

#![cfg(all(feature = "rust_decimal", feature = "uuid", feature = "json"))]
#![allow(dead_code)]

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{Postgres, Value};

use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Entity)]
#[oxider(table = "invoices")]
struct Invoice {
    id: Uuid,
    total: Decimal,
    metadata: serde_json::Value,
}

/// A decimal binds as its digits, not as a float. `0.1 + 0.2` is the reason:
/// widening to `f64` anywhere on this path would make an exact type inexact.
#[test]
fn a_decimal_binds_the_digits_it_was_written_with() {
    let query = Invoice::query().filter(Invoice::total.gt(Decimal::from_str("1234.5678").unwrap()));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" WHERE "invoices"."total" > $1"#,
        &[Value::Decimal("1234.5678".into())],
    );
}

/// Trailing zeros are part of an exact decimal's scale, so they survive too.
#[test]
fn a_decimals_scale_survives_binding() {
    let query = Invoice::query().filter(Invoice::total.eq(Decimal::from_str("10.00").unwrap()));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" WHERE "invoices"."total" = $1"#,
        &[Value::Decimal("10.00".into())],
    );
}

/// A decimal is `Numeric`, so it gets the arithmetic and the numeric aggregates.
#[test]
fn a_decimal_column_supports_arithmetic_and_aggregates() {
    let query = Invoice::query().select(Invoice::total.sum().alias("owed"));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT SUM("invoices"."total") AS "owed" FROM "invoices""#,
    );

    // The right-hand side has to be a decimal too. An integer literal does not
    // widen into one, exactly as it does not widen into `f64`: the widening list
    // is explicit, because making every numeric position accept every numeric
    // type is what makes inference ambiguous everywhere else.
    let query =
        Invoice::query().filter(Invoice::total.mul(Decimal::from(2)).gt(Decimal::from(100)));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" WHERE "invoices"."total" * $1 > $2"#,
        &[Value::Decimal("2".into()), Value::Decimal("100".into())],
    );
}

#[test]
fn a_uuid_binds_as_its_hyphenated_form() {
    let id = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    let query = Invoice::query().filter(Invoice::id.eq(id));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" WHERE "invoices"."id" = $1"#,
        &[Value::Uuid("67e55044-10b1-426f-9247-bb680e5fe0c8".into())],
    );
}

/// UUIDs order, so keyset paging over one compiles.
#[test]
fn a_uuid_column_orders() {
    let query = Invoice::query().order_by(Invoice::id.asc()).limit(10);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" ORDER BY "invoices"."id" ASC LIMIT 10"#,
    );
}

#[test]
fn json_binds_as_compact_serialised_text() {
    let document = serde_json::json!({ "plan": "pro", "seats": 5 });
    let query = Invoice::query().filter(Invoice::metadata.eq(document));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" WHERE "invoices"."metadata" = $1"#,
        &[Value::Json(r#"{"plan":"pro","seats":5}"#.into())],
    );
}

/// Equality and null tests are all a JSON column gets. It has no total order and
/// no arithmetic, so offering `<` or `SUM` would be offering something the
/// engines do not agree about.
///
/// ```compile_fail
/// # use oxider_query::prelude::*;
/// # #[derive(Entity)]
/// # #[oxider(table = "invoices")]
/// # struct Invoice { id: i64, metadata: serde_json::Value }
/// Invoice::query().order_by(Invoice::metadata.asc());
/// ```
#[test]
fn a_json_column_still_tests_for_null() {
    let query = Invoice::query().filter(Invoice::metadata.is_not_null());
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT * FROM "invoices" WHERE "invoices"."metadata" IS NOT NULL"#,
    );
}

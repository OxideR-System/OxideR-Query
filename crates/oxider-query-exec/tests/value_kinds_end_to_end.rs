//! Decimals, UUIDs and JSON against a real SQLite database.
//!
//! SQLite has a type for none of the three, so all three bind as the canonical
//! text `Value` carries. That is exactly where it could go quietly wrong, which
//! is why it is worth running rather than only rendering: text compared as text
//! sorts `"10.25"` before `"9.5"`, and a decimal column that did that would be
//! useless while looking fine.
//!
//! # Decimals on SQLite: pick one
//!
//! These tests pin a limitation rather than hide it. SQLite applies the column's
//! affinity to the bound text, and that decides which of two things you get:
//!
//! - **`NUMERIC` affinity**: the text becomes a number, so comparing and
//!   ordering behave arithmetically - and a value too wide for an integer
//!   becomes a `REAL`, which is a float, which is not exact.
//! - **`TEXT` affinity**: the digits survive exactly, and every comparison is
//!   lexicographic, so `"9.5" > "10.25"`.
//!
//! There is no third option; SQLite has no exact decimal. PostgreSQL and MySQL
//! both do, so neither has to choose. Anyone storing money in SQLite is better
//! off keeping minor units in an `INTEGER` than reaching for either of these.
//!
//! Nothing in this crate does the converting: a decimal leaves it as its digits.
//! What converts is the column.

#![cfg(all(
    feature = "sqlite",
    feature = "rust_decimal",
    feature = "uuid",
    feature = "json"
))]
#![allow(dead_code)]

use oxider_query::prelude::*;
use oxider_query_exec::SqliteDb;

use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

/// `total` is `NUMERIC`: that affinity is what makes SQLite turn the bound text
/// into a number, and so what makes comparing one order it.
#[derive(Entity)]
#[oxider(table = "invoices")]
struct Invoice {
    id: i64,
    reference: Uuid,
    total: Decimal,
    metadata: serde_json::Value,
}

/// The same money in a `TEXT` column, where the digits survive and the ordering
/// does not.
#[derive(Entity)]
#[oxider(table = "exact_invoices")]
struct ExactInvoice {
    id: i64,
    total: Decimal,
}

/// Just the key, for the tests that assert on which rows came back and in which
/// order. Reading `total` back is a separate question with its own answer per
/// column type, so it stays out of these.
#[derive(sqlx::FromRow, Debug, PartialEq)]
struct Id {
    id: i64,
}

/// One text column, for reading back what was actually stored.
#[derive(sqlx::FromRow, Debug, PartialEq)]
struct Stored {
    value: String,
}

async fn database() -> SqliteDb {
    let db = SqliteDb::connect("sqlite::memory:").await.unwrap();
    for statement in [
        "CREATE TABLE invoices (id INTEGER PRIMARY KEY, reference TEXT NOT NULL, \
         total NUMERIC NOT NULL, metadata TEXT NOT NULL)",
        "CREATE TABLE exact_invoices (id INTEGER PRIMARY KEY, total TEXT NOT NULL)",
    ] {
        sqlx::query(statement).execute(db.pool()).await.unwrap();
    }
    db
}

async fn add_invoice(db: &SqliteDb, id: i64, total: &str) {
    db.execute(
        Invoice::insert()
            .set(Invoice::id, id)
            .set(Invoice::reference, Uuid::from_u128(id as u128))
            .set(Invoice::total, Decimal::from_str(total).unwrap())
            .set(Invoice::metadata, serde_json::json!({ "n": id })),
    )
    .await
    .unwrap();
}

async fn add_exact(db: &SqliteDb, id: i64, total: &str) {
    db.execute(
        ExactInvoice::insert()
            .set(ExactInvoice::id, id)
            .set(ExactInvoice::total, Decimal::from_str(total).unwrap()),
    )
    .await
    .unwrap();
}

/// The test that would fail if a decimal were compared as text: `"10.25"` sorts
/// before `"9.5"` as a string and after it as a number.
#[tokio::test]
async fn a_decimal_orders_as_a_number_in_a_numeric_column() {
    let db = database().await;
    add_invoice(&db, 1, "10.25").await;
    add_invoice(&db, 2, "9.5").await;
    add_invoice(&db, 3, "100.00").await;

    let ascending: Vec<Id> = db
        .fetch_all(
            Invoice::query()
                .select(Invoice::id)
                .order_by(Invoice::total.asc()),
        )
        .await
        .unwrap();
    assert_eq!(
        ascending.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 1, 3],
        "9.5 < 10.25 < 100.00"
    );

    let above: Vec<Id> = db
        .fetch_all(
            Invoice::query()
                .select(Invoice::id)
                .filter(Invoice::total.gt(Decimal::from_str("10").unwrap()))
                .order_by(Invoice::id.asc()),
        )
        .await
        .unwrap();
    assert_eq!(
        above.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3],
        "comparing binds a number too"
    );
}

/// The other half of the trade-off: in a `TEXT` column the digits arrive
/// exactly, because nothing converts them.
#[tokio::test]
async fn a_decimal_keeps_its_digits_in_a_text_column() {
    let db = database().await;
    let exact = "12345678901234.567890";
    add_exact(&db, 1, exact).await;

    let rows: Vec<Stored> = db
        .fetch_all(ExactInvoice::query().select(ExactInvoice::total.alias("value")))
        .await
        .unwrap();
    assert_eq!(rows[0].value, exact, "every digit, including the scale");
    assert_eq!(
        Decimal::from_str(&rows[0].value).unwrap(),
        Decimal::from_str(exact).unwrap()
    );
}

/// And what that column costs. Pinned so the test above is not read as a
/// recommendation without seeing what it gives up.
#[tokio::test]
async fn a_text_column_orders_decimals_lexicographically() {
    let db = database().await;
    add_exact(&db, 1, "10.25").await;
    add_exact(&db, 2, "9.5").await;

    let ascending: Vec<Id> = db
        .fetch_all(
            ExactInvoice::query()
                .select(ExactInvoice::id)
                .order_by(ExactInvoice::total.asc()),
        )
        .await
        .unwrap();
    assert_eq!(
        ascending.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2],
        r#""10.25" sorts before "9.5" as text, which is why NUMERIC exists"#
    );
}

#[tokio::test]
async fn a_uuid_round_trips_as_its_hyphenated_form() {
    let db = database().await;
    let reference = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    db.execute(
        Invoice::insert()
            .set(Invoice::id, 1)
            .set(Invoice::reference, reference)
            .set(Invoice::total, Decimal::from(1))
            .set(Invoice::metadata, serde_json::json!({})),
    )
    .await
    .unwrap();

    let rows: Vec<Stored> = db
        .fetch_all(Invoice::query().select(Invoice::reference.alias("value")))
        .await
        .unwrap();
    assert_eq!(rows[0].value, "67e55044-10b1-426f-9247-bb680e5fe0c8");

    // And it finds itself again, which is what a UUID is for.
    let found: Vec<Id> = db
        .fetch_all(
            Invoice::query()
                .select(Invoice::id)
                .filter(Invoice::reference.eq(reference)),
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
}

#[tokio::test]
async fn json_round_trips_as_a_document() {
    let db = database().await;
    let document = serde_json::json!({ "plan": "pro", "seats": 5, "tags": ["a", "b"] });
    db.execute(
        Invoice::insert()
            .set(Invoice::id, 1)
            .set(Invoice::reference, Uuid::nil())
            .set(Invoice::total, Decimal::from(1))
            .set(Invoice::metadata, document.clone()),
    )
    .await
    .unwrap();

    let rows: Vec<Stored> = db
        .fetch_all(Invoice::query().select(Invoice::metadata.alias("value")))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rows[0].value).unwrap(),
        document
    );
}

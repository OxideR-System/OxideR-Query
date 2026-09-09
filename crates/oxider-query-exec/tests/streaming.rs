//! Walking a result set row by row instead of collecting it.
//!
//! `for_each_row` exists for the case where `fetch_all` would build a `Vec` too
//! large to want in memory. That is hard to assert directly - a test proving a
//! million rows fit would take a million rows - so what these pin instead is
//! everything observable about the walk: that every row arrives, in the query's
//! order, that the count is right, that an error from the closure stops it
//! early, and that a transaction can walk too.

#![cfg(feature = "sqlite")]

use oxider_query::prelude::*;
use oxider_query_exec::{Error, Result, SqliteDb};

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "readings")]
struct Reading {
    id: i64,
    value: i64,
}

/// One column, read by position.
#[derive(Debug, PartialEq)]
struct Value(i64);

impl<'r, R> oxider_query_exec::Projection<'r, R> for Value
where
    R: sqlx::Row,
    usize: sqlx::ColumnIndex<R>,
    i64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    const ARITY: usize = 1;

    fn from_row_at(row: &'r R, offset: usize) -> std::result::Result<Self, sqlx::Error> {
        Ok(Value(sqlx::Row::try_get(row, offset)?))
    }
}

async fn database(rows: i64) -> SqliteDb {
    let db = SqliteDb::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE readings (id INTEGER PRIMARY KEY, value INTEGER NOT NULL)")
        .execute(db.pool())
        .await
        .unwrap();
    for id in 1..=rows {
        db.execute(
            Reading::insert()
                .set(Reading::id, id)
                .set(Reading::value, id * 10),
        )
        .await
        .unwrap();
    }
    db
}

#[tokio::test]
async fn every_row_arrives_in_the_querys_order() {
    let db = database(5).await;

    let mut seen = Vec::new();
    let count = db
        .for_each_row(
            Reading::query().order_by(Reading::id.desc()),
            |row: Reading| {
                seen.push(row.id);
                Ok(())
            },
        )
        .await
        .unwrap();

    assert_eq!(count, 5, "the count is the number of rows walked");
    assert_eq!(seen, vec![5, 4, 3, 2, 1], "the query's order is kept");
}

/// The closure can accumulate, which is the whole point: a summary over more
/// rows than would fit costs one number rather than a `Vec`.
#[tokio::test]
async fn the_closure_can_fold_without_holding_the_rows() {
    let db = database(100).await;

    let mut total = 0i64;
    let count = db
        .for_each_row(Reading::query(), |row: Reading| {
            total += row.value;
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(count, 100);
    assert_eq!(total, (1..=100).map(|n| n * 10).sum::<i64>());
}

/// An error from the closure stops the walk where it happened, so a caller
/// looking for one row does not pay for the rest.
#[tokio::test]
async fn an_error_from_the_closure_stops_the_walk() {
    let db = database(1000).await;

    let mut seen = 0;
    let result: Result<u64> = db
        .for_each_row(
            Reading::query().order_by(Reading::id.asc()),
            |row: Reading| {
                seen += 1;
                if row.id == 3 {
                    return Err(Error::Render(oxider_query_core::RenderError::Invalid(
                        "found it",
                    )));
                }
                Ok(())
            },
        )
        .await;

    assert!(result.is_err(), "the closure's error is the result");
    assert_eq!(seen, 3, "and nothing after the third row was read");
}

#[tokio::test]
async fn an_empty_result_walks_zero_rows() {
    let db = database(0).await;

    let mut called = false;
    let count = db
        .for_each_row(Reading::query(), |_: Reading| {
            called = true;
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(count, 0);
    assert!(!called, "the closure is not called at all");
}

/// The positional form reads the `select` list in order, like the rest of the
/// projected API, rather than matching column names.
#[tokio::test]
async fn the_projected_form_reads_by_position() {
    let db = database(3).await;

    let mut seen = Vec::new();
    let count = db
        .for_each_row_projected(
            Reading::query()
                .select(Reading::value)
                .order_by(Reading::id.asc()),
            |row: Value| {
                seen.push(row.0);
                Ok(())
            },
        )
        .await
        .unwrap();

    assert_eq!(count, 3);
    assert_eq!(seen, vec![10, 20, 30]);
}

/// Walking inside a transaction sees that transaction's own uncommitted writes,
/// which is what makes it usable for a migration that reads and writes.
#[tokio::test]
async fn a_transaction_walks_its_own_uncommitted_rows() {
    let db = database(2).await;
    let mut tx = db.begin().await.unwrap();

    tx.execute(
        Reading::insert()
            .set(Reading::id, 99)
            .set(Reading::value, 990),
    )
    .await
    .unwrap();

    let mut seen = Vec::new();
    let count = tx
        .for_each_row(
            Reading::query().order_by(Reading::id.asc()),
            |row: Reading| {
                seen.push(row.id);
                Ok(())
            },
        )
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    assert_eq!(count, 3);
    assert_eq!(seen, vec![1, 2, 99]);

    // And after the rollback the row is gone, so the walk really was inside it.
    let after: Vec<Reading> = db.fetch_all(Reading::query()).await.unwrap();
    assert_eq!(after.len(), 2);
}

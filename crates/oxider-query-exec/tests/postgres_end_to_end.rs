//! End-to-end scenarios against a real PostgreSQL database.
//!
//! Skipped unless `OXIDER_POSTGRES_URL` is set, so the default `cargo test` run
//! needs no server. Start one with:
//!
//! ```text
//! docker run -d --name oxider-pg -e POSTGRES_PASSWORD=pw -p 54329:5432 postgres:16
//! export OXIDER_POSTGRES_URL=postgres://postgres:pw@localhost:54329/postgres
//! ```
//!
//! These exist because Postgres is the strict backend: it carries a type per
//! parameter rather than inferring one from the value, so encoding mistakes
//! that SQLite silently accepts are errors here.

#![cfg(feature = "postgres")]

use oxider_query::prelude::*;
use oxider_query_exec::{PostgresDb, Result};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "ox_users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
    age: i64,
    active: bool,
}

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "ox_events")]
struct Event {
    id: i64,
    label: String,
    on_day: NaiveDate,
    at_time: NaiveTime,
    happened_at: NaiveDateTime,
}

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "ox_instants")]
struct Instant {
    id: i64,
    seen_at: DateTime<Utc>,
}

/// One projected column, for queries that select a single expression.
#[derive(sqlx::FromRow, Debug, PartialEq)]
struct One<T> {
    value: T,
}

/// The error a transaction test aborts with.
///
/// A distinct variant from `Database`, so the assertion proves the caller's own
/// error came back rather than something the rollback produced.
#[derive(Debug, PartialEq)]
enum Abort {
    Deliberate,
    Database(String),
}

impl From<oxider_query_exec::Error> for Abort {
    fn from(err: oxider_query_exec::Error) -> Self {
        Abort::Database(err.to_string())
    }
}

/// The server to test against, or `None` when the suite should skip.
fn url() -> Option<String> {
    std::env::var("OXIDER_POSTGRES_URL").ok()
}

/// Serialises the whole suite.
///
/// Every test drops and recreates the same tables, so running two at once means
/// one clearing the other's rows mid-assertion, or both issuing the DDL and one
/// losing with `relation "ox_users" already exists`. Unlike the SQLite suite,
/// where each test gets a private in-memory database for free, these share one
/// server. Serialising is the honest fix; passing `--test-threads=1` would only
/// hide it from whoever forgets the flag.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A connected handle over freshly created tables, plus the guard that keeps
/// this test alone on the server until it finishes.
///
/// Tables are dropped and recreated rather than shared, so the suite is
/// order-independent and rerunnable against the same server.
async fn database() -> Option<(PostgresDb, tokio::sync::MutexGuard<'static, ()>)> {
    let guard = ONE_AT_A_TIME.lock().await;
    let db = PostgresDb::connect(&url()?).await.unwrap();
    for statement in [
        "DROP TABLE IF EXISTS ox_users",
        "DROP TABLE IF EXISTS ox_events",
        "DROP TABLE IF EXISTS ox_instants",
        "CREATE TABLE ox_users (id BIGINT PRIMARY KEY, name TEXT NOT NULL, email TEXT, \
         age BIGINT NOT NULL, active BOOLEAN NOT NULL)",
        "CREATE TABLE ox_events (id BIGINT PRIMARY KEY, label TEXT NOT NULL, \
         on_day DATE NOT NULL, at_time TIME NOT NULL, happened_at TIMESTAMP NOT NULL)",
        "CREATE TABLE ox_instants (id BIGINT PRIMARY KEY, seen_at TIMESTAMPTZ NOT NULL)",
    ] {
        sqlx::query(statement).execute(db.pool()).await.unwrap();
    }
    Some((db, guard))
}

/// Bind a handle for the test body, or skip when no server is configured.
///
/// A statement macro rather than an expression one, because the guard has to be
/// bound in the caller's scope: bound inside an expression it would drop as soon
/// as that expression finished, leaving the suite unserialised while every
/// comment here claimed otherwise.
macro_rules! db_or_skip {
    ($db:ident) => {
        let ($db, _guard) = match database().await {
            Some(pair) => pair,
            None => {
                eprintln!("OXIDER_POSTGRES_URL is not set, skipping");
                return;
            }
        };
    };
}

async fn add_user(db: &PostgresDb, id: i64, name: &str, age: i64) -> Result<u64> {
    db.execute(
        User::insert()
            .set(User::id, id)
            .set(User::name, name.to_string())
            .set(User::age, age)
            .set(User::active, true),
    )
    .await
}

#[tokio::test]
async fn an_insert_round_trips_through_a_select() {
    db_or_skip!(db);
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_user(&db, 2, "grace", 45).await.unwrap();

    let found: Vec<User> = db
        .fetch_all(User::query().filter(User::age.ge(40)))
        .await
        .unwrap();
    assert_eq!(
        found,
        vec![User {
            id: 2,
            name: "grace".into(),
            email: None,
            age: 45,
            active: true,
        }]
    );
}

/// The reason this suite exists.
///
/// `Value` stores dates, times and timestamps as the formatted strings the
/// renderer would have written. SQLite takes those happily because it types a
/// column by the value it is handed. Postgres carries a type OID per parameter
/// and refuses text where a `date` is expected, so the backend has to hand it
/// a real temporal rather than the string.
#[tokio::test]
async fn temporal_values_bind_as_the_types_the_columns_actually_are() {
    db_or_skip!(db);
    let day = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
    let time = NaiveTime::from_hms_opt(9, 30, 0).unwrap();
    let stamp = day.and_hms_opt(9, 30, 0).unwrap();

    db.execute(
        Event::insert()
            .set(Event::id, 1)
            .set(Event::label, "launch".to_string())
            .set(Event::on_day, day)
            .set(Event::at_time, time)
            .set(Event::happened_at, stamp),
    )
    .await
    .unwrap();

    let found: Vec<Event> = db.fetch_all(Event::query()).await.unwrap();
    assert_eq!(
        found,
        vec![Event {
            id: 1,
            label: "launch".into(),
            on_day: day,
            at_time: time,
            happened_at: stamp,
        }]
    );

    // Filtering on a temporal binds through the same path, and comparing has to
    // happen as dates rather than as text for the ordering to mean anything.
    let later: Vec<Event> = db
        .fetch_all(
            Event::query().filter(Event::on_day.gt(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())),
        )
        .await
        .unwrap();
    assert_eq!(later.len(), 1);

    let none: Vec<Event> = db
        .fetch_all(Event::query().filter(Event::happened_at.lt(stamp)))
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn the_postgres_only_clauses_run_where_they_were_rendered_for() {
    db_or_skip!(db);
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_user(&db, 2, "grace", 45).await.unwrap();
    add_user(&db, 3, "ada", 20).await.unwrap();

    // DISTINCT ON is refused by both other dialects, so this is the first time
    // it reaches a server that can run it.
    let first_per_name: Vec<User> = db
        .fetch_all(
            User::query()
                .distinct_on(User::name)
                .order_by(User::name.asc())
                .order_by(User::age.desc()),
        )
        .await
        .unwrap();
    assert_eq!(
        first_per_name.iter().map(|u| u.id).collect::<Vec<_>>(),
        vec![1, 2],
        "one row per name, the oldest of each"
    );

    // RETURNING on an UPDATE, and a native FILTER on an aggregate.
    let returned: Vec<One<String>> = db
        .fetch_all(
            User::update()
                .set(User::active, false)
                .filter(User::id.eq(3))
                .returning(User::name.alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(
        returned,
        vec![One {
            value: "ada".into()
        }]
    );

    let adults: One<i64> = db
        .fetch_one(User::query().select(count_all().filter_where(User::age.ge(21)).alias("value")))
        .await
        .unwrap();
    assert_eq!(adults.value, 2);
}

#[tokio::test]
async fn a_named_parameter_binds_the_same_way_a_literal_does() {
    db_or_skip!(db);
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_user(&db, 2, "grace", 45).await.unwrap();

    let template = User::query().filter(User::age.ge(param::<i64>("floor")));
    let older: Vec<User> = db
        .fetch_all(template.clone().bind("floor", 40))
        .await
        .unwrap();
    assert_eq!(older.len(), 1);

    let everyone: Vec<User> = db.fetch_all(template.bind("floor", 0)).await.unwrap();
    assert_eq!(everyone.len(), 2);
}

#[tokio::test]
async fn a_transaction_that_returns_an_error_rolls_back() {
    db_or_skip!(db);
    add_user(&db, 1, "ada", 36).await.unwrap();

    let outcome: std::result::Result<(), Abort> = db
        .transaction(async |tx| {
            tx.execute(
                User::update()
                    .set(User::name, "changed".to_string())
                    .filter(User::id.eq(1)),
            )
            .await
            .unwrap();
            Err(Abort::Deliberate)
        })
        .await;
    assert_eq!(outcome.unwrap_err(), Abort::Deliberate);

    let still: Vec<User> = db
        .fetch_all(User::query().filter(User::id.eq(1)))
        .await
        .unwrap();
    assert_eq!(still[0].name, "ada", "the update was rolled back");
}

/// An instant is the one temporal written with an explicit offset, so it parses
/// back by a different route than the naive kinds and needs its own coverage.
/// A zoned column is also where getting this wrong is worst: text would be
/// rejected outright, but a naive value silently reinterpreted in the session's
/// zone would be accepted and wrong.
#[tokio::test]
async fn an_instant_binds_to_a_zoned_column_without_losing_its_offset() {
    db_or_skip!(db);
    let seen = Utc.with_ymd_and_hms(2024, 3, 15, 9, 30, 0).unwrap();

    db.execute(
        Instant::insert()
            .set(Instant::id, 1)
            .set(Instant::seen_at, seen),
    )
    .await
    .unwrap();

    let found: Vec<Instant> = db.fetch_all(Instant::query()).await.unwrap();
    assert_eq!(
        found,
        vec![Instant {
            id: 1,
            seen_at: seen
        }]
    );

    // The session's zone must not shift it. Asking Postgres for the instant in
    // UTC has to give back exactly what went in, whatever the server's default.
    sqlx::query("SET TIME ZONE 'Asia/Tokyo'")
        .execute(db.pool())
        .await
        .unwrap();
    let after_shift: Vec<Instant> = db
        .fetch_all(Instant::query().filter(Instant::seen_at.eq(seen)))
        .await
        .unwrap();
    assert_eq!(after_shift.len(), 1, "the instant still matches itself");
}

/// The strict backend is where binding a decimal, a UUID or a JSON document as
/// text would be rejected outright rather than quietly coerced: Postgres carries
/// a type per parameter and `numeric`, `uuid` and `jsonb` are all real types
/// here. So this is the test that proves the backend hands the driver a typed
/// value rather than the text `Value` carries.
#[cfg(all(feature = "rust_decimal", feature = "uuid", feature = "json"))]
#[tokio::test]
async fn decimals_uuids_and_json_bind_as_the_native_types() {
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use uuid::Uuid;

    #[derive(oxider_query::Entity, sqlx::FromRow, Debug, PartialEq)]
    #[oxider(table = "ox_invoices")]
    struct Invoice {
        id: Uuid,
        total: Decimal,
        metadata: serde_json::Value,
    }

    db_or_skip!(db);
    sqlx::query("DROP TABLE IF EXISTS ox_invoices")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE ox_invoices (id UUID PRIMARY KEY, total NUMERIC(30, 10) NOT NULL, \
         metadata JSONB NOT NULL)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let id = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
    // Wider than an f64 can hold exactly, which is the point of a decimal.
    let total = Decimal::from_str("12345678901234.5678901234").unwrap();
    let metadata = serde_json::json!({ "plan": "pro", "seats": 5 });

    db.execute(
        Invoice::insert()
            .set(Invoice::id, id)
            .set(Invoice::total, total)
            .set(Invoice::metadata, metadata.clone()),
    )
    .await
    .unwrap();

    let found: Vec<Invoice> = db.fetch_all(Invoice::query()).await.unwrap();
    assert_eq!(
        found,
        vec![Invoice {
            id,
            total,
            metadata
        }],
        "every digit survives a NUMERIC column"
    );

    // Comparing has to bind a numeric too, or the server would refuse it.
    let above: Vec<Invoice> = db
        .fetch_all(Invoice::query().filter(Invoice::total.gt(Decimal::from(1))))
        .await
        .unwrap();
    assert_eq!(above.len(), 1);

    let by_id: Vec<Invoice> = db
        .fetch_all(Invoice::query().filter(Invoice::id.eq(id)))
        .await
        .unwrap();
    assert_eq!(by_id.len(), 1, "a uuid parameter matches a uuid column");

    sqlx::query("DROP TABLE ox_invoices")
        .execute(db.pool())
        .await
        .unwrap();
}

/// The ordered-set aggregates, against the one engine that has them.
///
/// Rendering tests already pin the SQL text. What they cannot show is that
/// PostgreSQL parses `WITHIN GROUP` where this puts it, or that the two
/// percentiles differ the way their names say: over four ages, the continuous
/// one interpolates a value that is in no row, and the discrete one returns a
/// value that is.
#[tokio::test]
async fn the_two_percentiles_interpolate_and_do_not() {
    db_or_skip!(db);
    for (id, age) in [(1, 10), (2, 20), (3, 30), (4, 40)] {
        add_user(&db, id, "u", age).await.unwrap();
    }

    let interpolated: Vec<One<f64>> = db
        .fetch_all(
            User::query().select(percentile_cont(0.5).within_group(User::age).alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(
        interpolated[0].value, 25.0,
        "halfway between the two middle ages, which no row holds"
    );

    let taken: Vec<One<i64>> = db
        .fetch_all(
            User::query().select(percentile_disc(0.5).within_group(User::age).alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(
        taken[0].value, 20,
        "the first age at or past the halfway mark, exactly as stored"
    );

    // Sorting the group the other way reaches the other side of the middle.
    let descending: Vec<One<i64>> = db
        .fetch_all(
            User::query().select(
                percentile_disc(0.5)
                    .within_group_desc(User::age)
                    .alias("value"),
            ),
        )
        .await
        .unwrap();
    assert_eq!(descending[0].value, 30);
}

/// A percentile per group, with the sort inside the aggregate and a separate
/// sort outside it. The two ORDER BY clauses mean different things and the
/// engine has to read them as such.
#[tokio::test]
async fn a_percentile_per_group_keeps_its_sort_separate_from_the_querys() {
    db_or_skip!(db);
    for (id, name, age) in [(1, "a", 10), (2, "a", 30), (3, "b", 50), (4, "b", 70)] {
        add_user(&db, id, name, age).await.unwrap();
    }

    #[derive(sqlx::FromRow, Debug, PartialEq)]
    struct Row {
        name: String,
        median: f64,
    }

    let rows: Vec<Row> = db
        .fetch_all(
            User::query()
                .select((
                    User::name,
                    percentile_cont(0.5).within_group(User::age).alias("median"),
                ))
                .group_by(User::name)
                .order_by(User::name.desc()),
        )
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![
            Row {
                name: "b".into(),
                median: 60.0
            },
            Row {
                name: "a".into(),
                median: 20.0
            },
        ]
    );
}

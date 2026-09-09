//! End-to-end scenarios against a real MySQL database.
//!
//! Skipped unless `OXIDER_MYSQL_URL` is set, so the default `cargo test` run
//! needs no server. Start one with:
//!
//! ```text
//! docker run -d --name oxider-mysql -e MYSQL_ROOT_PASSWORD=oxider \
//!     -e MYSQL_DATABASE=oxider -p 33069:3306 mysql:8
//! export OXIDER_MYSQL_URL=mysql://root:oxider@localhost:33069/oxider
//! ```
//!
//! These exist because MySQL is the dialect that emulates the most. It has no
//! `NULLS LAST`, no aggregate `FILTER`, no `RETURNING`, no `INTERSECT` or
//! `EXCEPT` worth relying on, and no zoned datetime type. Every one of those is
//! either rewritten into something equivalent or refused, and a test that only
//! pinned the rendered string would prove neither.

#![cfg(feature = "mysql")]

use oxider_query::prelude::*;
use oxider_query_exec::{MySqlDb, Result};

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
#[oxider(table = "ox_posts")]
struct Post {
    id: i64,
    user_id: i64,
    title: String,
    views: i64,
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
    std::env::var("OXIDER_MYSQL_URL").ok()
}

/// Serialises the whole suite.
///
/// Every test drops and recreates the same tables, so running two at once means
/// one clearing the other's rows mid-assertion. Unlike the SQLite suite, where
/// each test gets a private in-memory database for free, these share one server.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A connected handle over freshly created tables, plus the guard that keeps
/// this test alone on the server until it finishes.
///
/// `name` and `email` are `VARCHAR` rather than `TEXT` because MySQL will not
/// index a `TEXT` column without a prefix length, and the upsert scenario needs
/// a unique key on `email` for `ON DUPLICATE KEY UPDATE` to have anything to
/// duplicate.
///
/// `seen_at` is `DATETIME`, not `TIMESTAMP`. See the backend's module docs: an
/// instant binds as its UTC wall clock, which a `TIMESTAMP` column would
/// reinterpret through the connection's `time_zone`.
async fn database() -> Option<(MySqlDb, tokio::sync::MutexGuard<'static, ()>)> {
    let guard = ONE_AT_A_TIME.lock().await;
    let db = MySqlDb::connect(&url()?).await.unwrap();
    for statement in [
        "DROP TABLE IF EXISTS ox_users",
        "DROP TABLE IF EXISTS ox_posts",
        "DROP TABLE IF EXISTS ox_events",
        "DROP TABLE IF EXISTS ox_instants",
        "CREATE TABLE ox_users (id BIGINT PRIMARY KEY, name VARCHAR(255) NOT NULL, \
         email VARCHAR(255) UNIQUE, age BIGINT NOT NULL, active BOOLEAN NOT NULL)",
        "CREATE TABLE ox_posts (id BIGINT PRIMARY KEY, user_id BIGINT NOT NULL, \
         title VARCHAR(255) NOT NULL, views BIGINT NOT NULL)",
        "CREATE TABLE ox_events (id BIGINT PRIMARY KEY, label VARCHAR(255) NOT NULL, \
         on_day DATE NOT NULL, at_time TIME NOT NULL, happened_at DATETIME NOT NULL)",
        "CREATE TABLE ox_instants (id BIGINT PRIMARY KEY, seen_at DATETIME NOT NULL)",
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
                eprintln!("OXIDER_MYSQL_URL is not set, skipping");
                return;
            }
        };
    };
}

async fn add_user(db: &MySqlDb, id: i64, name: &str, email: Option<&str>, age: i64) -> Result<u64> {
    db.execute(
        User::insert()
            .set(User::id, id)
            .set(User::name, name.to_string())
            .set(User::email, email.map(str::to_string))
            .set(User::age, age)
            .set(User::active, true),
    )
    .await
}

#[tokio::test]
async fn an_insert_round_trips_through_a_select() {
    db_or_skip!(db);
    add_user(&db, 1, "ada", Some("ada@example.com"), 36)
        .await
        .unwrap();
    add_user(&db, 2, "grace", None, 45).await.unwrap();

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

/// MySQL takes a date sent as text, so this would pass even if the backend
/// bound the raw string. It is here for the comparison at the end: text sorts
/// as text, so a filter that agrees with a date comparison proves the parameter
/// arrived as a date.
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

/// MySQL has no zoned datetime type, so an instant loses its offset on the way
/// in. This pins what it keeps: the UTC wall clock, unchanged, whatever the
/// session's `time_zone` happens to be.
///
/// The session is deliberately moved off UTC first. A `DATETIME` column stores
/// what it is handed without conversion, so the round trip must survive that;
/// a `TIMESTAMP` column would not, which is why the backend docs say to use
/// `DATETIME` for instants.
#[tokio::test]
async fn an_instant_binds_to_a_datetime_as_its_utc_wall_clock() {
    db_or_skip!(db);
    sqlx::query("SET time_zone = '+09:00'")
        .execute(db.pool())
        .await
        .unwrap();

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
        }],
        "a DATETIME must not be shifted by the session zone"
    );

    let matched: Vec<Instant> = db
        .fetch_all(Instant::query().filter(Instant::seen_at.eq(seen)))
        .await
        .unwrap();
    assert_eq!(matched.len(), 1, "the instant still matches itself");
}

#[tokio::test]
async fn an_escaped_like_matches_the_literal_text_and_nothing_else() {
    // The bug this escaping exists to prevent: without it, searching for "50%"
    // matches "500 units" too, because `%` is a wildcard.
    db_or_skip!(db);
    db.execute(
        Post::insert()
            .columns((Post::id, Post::user_id, Post::title, Post::views))
            .values((1, 1, "50% off today", 0))
            .values((2, 1, "500 units shipped", 0))
            .values((3, 1, "a_b literal", 0))
            .values((4, 1, "axb wildcard", 0)),
    )
    .await
    .unwrap();

    let percent: Vec<Post> = db
        .fetch_all(Post::query().filter(Post::title.contains("50%")))
        .await
        .unwrap();
    assert_eq!(percent.len(), 1, "matched: {percent:?}");
    assert_eq!(percent[0].title, "50% off today");

    let underscore: Vec<Post> = db
        .fetch_all(Post::query().filter(Post::title.contains("a_b")))
        .await
        .unwrap();
    assert_eq!(underscore.len(), 1, "matched: {underscore:?}");
    assert_eq!(underscore[0].title, "a_b literal");

    // A pattern passed through `like` keeps its wildcards, so the same search
    // written that way matches both.
    let raw: Vec<Post> = db
        .fetch_all(Post::query().filter(Post::title.like("50%")))
        .await
        .unwrap();
    assert_eq!(raw.len(), 2, "matched: {raw:?}");
}

/// MySQL has no `NULLS FIRST`/`NULLS LAST`, so both orderings here run the
/// emulated `CASE WHEN ... IS NULL` sort key rather than a native clause.
#[tokio::test]
async fn emulated_null_ordering_puts_the_nulls_where_the_native_clause_would() {
    db_or_skip!(db);
    add_user(&db, 1, "with", Some("b@example.com"), 30)
        .await
        .unwrap();
    add_user(&db, 2, "without", None, 31).await.unwrap();
    add_user(&db, 3, "another", Some("a@example.com"), 32)
        .await
        .unwrap();

    let last: Vec<User> = db
        .fetch_all(User::query().order_by(User::email.asc().nulls_last()))
        .await
        .unwrap();
    assert_eq!(
        last.iter().map(|u| u.id).collect::<Vec<_>>(),
        vec![3, 1, 2],
        "the null sorts after every value"
    );

    let first: Vec<User> = db
        .fetch_all(User::query().order_by(User::email.asc().nulls_first()))
        .await
        .unwrap();
    assert_eq!(
        first.iter().map(|u| u.id).collect::<Vec<_>>(),
        vec![2, 3, 1],
        "the null sorts before every value"
    );
}

/// MySQL has no aggregate `FILTER`, so `filter_where` renders as a `CASE` folded
/// into the argument. This checks the emulation counts the rows the clause
/// describes, by comparing it against a plain `WHERE` over the same data.
#[tokio::test]
async fn an_emulated_aggregate_filter_counts_the_same_rows_as_a_where_clause() {
    db_or_skip!(db);
    db.execute(
        Post::insert()
            .columns((Post::id, Post::user_id, Post::title, Post::views))
            .values((1, 1, "a", 5))
            .values((2, 1, "b", 50))
            .values((3, 1, "c", 500)),
    )
    .await
    .unwrap();

    let rendered = Post::query()
        .select(count_all().filter_where(Post::views.ge(50)).alias("value"))
        .to_sql(&MySql)
        .unwrap();
    assert!(
        rendered.sql.contains("CASE WHEN"),
        "expected the emulation, got: {}",
        rendered.sql
    );

    let emulated: One<i64> = db
        .fetch_one(
            Post::query().select(count_all().filter_where(Post::views.ge(50)).alias("value")),
        )
        .await
        .unwrap();
    let plain: One<i64> = db
        .fetch_one(
            Post::query()
                .filter(Post::views.ge(50))
                .select(count_all().alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(
        emulated.value, plain.value,
        "the emulation must count the same rows"
    );
    assert_eq!(emulated.value, 2);
}

/// MySQL spells an upsert `ON DUPLICATE KEY UPDATE`, and names the incoming row
/// `VALUES(col)` rather than `excluded.col`. Both are rewrites of the same
/// builder call, so this proves the rewrite runs, not just that it renders.
#[tokio::test]
async fn an_upsert_inserts_once_and_updates_afterwards() {
    db_or_skip!(db);

    let upsert = |name: &'static str, age: i64| {
        User::insert()
            .set(User::id, 1)
            .set(User::name, name)
            .set(User::email, "ada@example.com")
            .set(User::age, age)
            .set(User::active, true)
            .on_conflict(["email"])
            .do_update()
            .set(User::name, oxider_query::builder::excluded(User::name))
            .set(User::age, oxider_query::builder::excluded(User::age))
            .end()
    };

    db.execute(upsert("ada", 36)).await.unwrap();
    db.execute(upsert("ada lovelace", 37)).await.unwrap();

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert_eq!(rows.len(), 1, "the second write must not add a row");
    assert_eq!(rows[0].name, "ada lovelace");
    assert_eq!(rows[0].age, 37);
}

#[tokio::test]
async fn a_named_parameter_binds_the_same_way_a_literal_does() {
    db_or_skip!(db);
    add_user(&db, 1, "ada", Some("ada@example.com"), 36)
        .await
        .unwrap();
    add_user(&db, 2, "grace", None, 45).await.unwrap();

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
    add_user(&db, 1, "ada", Some("ada@example.com"), 36)
        .await
        .unwrap();

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

/// What MySQL cannot express is refused at render time, so the caller gets an
/// error naming the missing clause rather than a syntax error from the server
/// pointing at whatever token came next.
#[tokio::test]
async fn statements_the_engine_cannot_run_are_refused_before_reaching_it() {
    db_or_skip!(db);

    // No RETURNING.
    let outcome: Result<Vec<One<String>>> = db
        .fetch_all(
            User::insert()
                .set(User::id, 9)
                .set(User::name, "ada")
                .set(User::age, 36)
                .set(User::active, true)
                .returning(User::name.alias("value")),
        )
        .await;
    match outcome {
        Err(oxider_query_exec::Error::Render(err)) => {
            assert!(
                err.to_string().to_uppercase().contains("RETURNING"),
                "{err}"
            );
        }
        other => panic!("expected a render error, got {other:?}"),
    }

    // No FULL JOIN.
    let outcome: Result<Vec<User>> = db
        .fetch_all(
            User::query()
                .full_join(Post::table(), Post::user_id.eq(User::id))
                .select(User::id),
        )
        .await;
    match outcome {
        Err(oxider_query_exec::Error::Render(err)) => {
            assert!(err.to_string().to_uppercase().contains("FULL"), "{err}");
        }
        other => panic!("expected a render error, got {other:?}"),
    }
}

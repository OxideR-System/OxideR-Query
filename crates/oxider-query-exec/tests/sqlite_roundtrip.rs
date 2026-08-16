//! End-to-end: build statements with OxideR-Query, run them on in-memory SQLite.
//!
//! Note there is no `.render(&Sqlite)` at any call site: the `Db` handle takes
//! the query directly and renders it with its backend's dialect.

use oxider_query::prelude::*;
use oxider_query_exec::SqliteDb;

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i64,
    active: bool,
}

async fn seed() -> SqliteDb {
    let db = SqliteDb::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL, active BOOLEAN NOT NULL)",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn insert_then_select_roundtrips() {
    let db = seed().await;

    let inserted = db
        .execute(
            User::insert()
                .value(User::id, 1)
                .value(User::name, "Alice")
                .value(User::age, 30)
                .value(User::active, true),
        )
        .await
        .unwrap();
    assert_eq!(inserted, 1);

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert_eq!(
        rows,
        vec![User {
            id: 1,
            name: "Alice".into(),
            age: 30,
            active: true,
        }]
    );
}

#[tokio::test]
async fn filter_binds_params_correctly() {
    let db = seed().await;
    for (id, name, age) in [(1, "Alice", 30), (2, "Bob", 41), (3, "Cara", 25)] {
        db.execute(
            User::insert()
                .value(User::id, id)
                .value(User::name, name)
                .value(User::age, age)
                .value(User::active, true),
        )
        .await
        .unwrap();
    }

    let adults: Vec<User> = db
        .fetch_all(
            User::query()
                .filter(User::age.ge(30))
                .order_by(User::age.asc()),
        )
        .await
        .unwrap();

    let names: Vec<&str> = adults.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(names, vec!["Alice", "Bob"]);
}

#[tokio::test]
async fn update_and_delete_affect_rows() {
    let db = seed().await;
    db.execute(
        User::insert()
            .value(User::id, 1)
            .value(User::name, "Alice")
            .value(User::age, 30)
            .value(User::active, true),
    )
    .await
    .unwrap();

    let updated = db
        .execute(User::update().set(User::age, 31).filter(User::id.eq(1)))
        .await
        .unwrap();
    assert_eq!(updated, 1);

    let one: User = db
        .fetch_one(User::query().filter(User::id.eq(1)))
        .await
        .unwrap();
    assert_eq!(one.age, 31);

    let deleted = db.execute(User::delete()).await.unwrap();
    assert_eq!(deleted, 1);

    let gone: Option<User> = db
        .fetch_optional(User::query().filter(User::id.eq(1)))
        .await
        .unwrap();
    assert!(gone.is_none());
}

fn new_user(id: i64) -> impl Renderable {
    User::insert()
        .value(User::id, id)
        .value(User::name, "Alice")
        .value(User::age, 30)
        .value(User::active, true)
}

#[tokio::test]
async fn transaction_commit_persists() {
    let db = seed().await;

    let mut tx = db.begin().await.unwrap();
    tx.execute(new_user(1)).await.unwrap();
    tx.execute(new_user(2)).await.unwrap();
    // Reads inside the transaction see its own uncommitted writes.
    let mid: Vec<User> = tx.fetch_all(User::query()).await.unwrap();
    assert_eq!(mid.len(), 2);
    tx.commit().await.unwrap();

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn transaction_rollback_discards() {
    let db = seed().await;

    let mut tx = db.begin().await.unwrap();
    tx.execute(new_user(1)).await.unwrap();
    tx.rollback().await.unwrap();

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert!(rows.is_empty());
}

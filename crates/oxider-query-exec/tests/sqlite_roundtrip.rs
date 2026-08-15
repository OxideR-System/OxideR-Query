//! End-to-end: build statements with OxideR-Query, run them on in-memory SQLite.

use oxider_query::prelude::*;
use oxider_query_exec::{execute, fetch_all, fetch_one, fetch_optional};
use sqlx::SqlitePool;

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i64,
    active: bool,
}

async fn seed() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL, active BOOLEAN NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn insert_then_select_roundtrips() {
    let pool = seed().await;

    let inserted = execute(
        &pool,
        &User::insert()
            .value(User::id, 1)
            .value(User::name, "Alice")
            .value(User::age, 30)
            .value(User::active, true)
            .render(&Sqlite),
    )
    .await
    .unwrap();
    assert_eq!(inserted, 1);

    let rows: Vec<User> = fetch_all(&pool, &User::query().render(&Sqlite))
        .await
        .unwrap();
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
    let pool = seed().await;
    for (id, name, age) in [(1, "Alice", 30), (2, "Bob", 41), (3, "Cara", 25)] {
        execute(
            &pool,
            &User::insert()
                .value(User::id, id)
                .value(User::name, name)
                .value(User::age, age)
                .value(User::active, true)
                .render(&Sqlite),
        )
        .await
        .unwrap();
    }

    let adults: Vec<User> = fetch_all(
        &pool,
        &User::query()
            .filter(User::age.ge(30))
            .order_by(User::age.asc())
            .render(&Sqlite),
    )
    .await
    .unwrap();

    let names: Vec<&str> = adults.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(names, vec!["Alice", "Bob"]);
}

#[tokio::test]
async fn update_and_delete_affect_rows() {
    let pool = seed().await;
    execute(
        &pool,
        &User::insert()
            .value(User::id, 1)
            .value(User::name, "Alice")
            .value(User::age, 30)
            .value(User::active, true)
            .render(&Sqlite),
    )
    .await
    .unwrap();

    let updated = execute(
        &pool,
        &User::update()
            .set(User::age, 31)
            .filter(User::id.eq(1))
            .render(&Sqlite),
    )
    .await
    .unwrap();
    assert_eq!(updated, 1);

    let one: User = fetch_one(&pool, &User::query().filter(User::id.eq(1)).render(&Sqlite))
        .await
        .unwrap();
    assert_eq!(one.age, 31);

    let deleted = execute(&pool, &User::delete().render(&Sqlite))
        .await
        .unwrap();
    assert_eq!(deleted, 1);

    let gone: Option<User> =
        fetch_optional(&pool, &User::query().filter(User::id.eq(1)).render(&Sqlite))
            .await
            .unwrap();
    assert!(gone.is_none());
}

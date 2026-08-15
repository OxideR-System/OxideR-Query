//! Introspect an in-memory SQLite schema and check the generated entity source.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

// One connection so every statement hits the same in-memory database.
async fn schema() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE users (\
            id INTEGER PRIMARY KEY, \
            name TEXT NOT NULL, \
            email TEXT, \
            active BOOLEAN NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE TABLE order_items (id INTEGER PRIMARY KEY, price REAL NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn generates_structs_for_all_tables() {
    let pool = schema().await;
    let source = oxider_query_codegen::generate_entities(&pool)
        .await
        .unwrap();

    // Tables ordered by name: order_items before users.
    let expected = "\
#[derive(Entity)]
#[oxider(table = \"order_items\")]
pub struct OrderItems {
    pub id: i64,
    pub price: f64,
}

#[derive(Entity)]
#[oxider(table = \"users\")]
pub struct Users {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub active: bool,
}
";
    assert_eq!(source, expected);
}

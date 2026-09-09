//! Introspect an in-memory SQLite schema and check the generated entity source.
//!
//! The generated text is pinned in full, and every case additionally parses it
//! with `syn`: source that looks right but does not parse is the failure mode
//! that matters, because the user only finds out when their own crate fails to
//! build.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// One connection so every statement hits the same in-memory database.
async fn database(statements: &[&str]) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for statement in statements {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    pool
}

/// Generate the entity source, asserting it is valid Rust before returning it.
#[track_caller]
fn valid_rust(source: &str) -> &str {
    if let Err(err) = syn::parse_file(source) {
        panic!("the generated source is not valid Rust: {err}\n---\n{source}---");
    }
    source
}

#[tokio::test]
async fn generates_structs_for_all_tables() {
    let pool = database(&[
        "CREATE TABLE users (\
            id INTEGER PRIMARY KEY, \
            name TEXT NOT NULL, \
            email TEXT, \
            active BOOLEAN NOT NULL)",
        "CREATE TABLE order_items (id INTEGER PRIMARY KEY, price REAL NOT NULL)",
    ])
    .await;
    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
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
    assert_eq!(valid_rust(&source), expected);
}

#[tokio::test]
async fn a_column_named_after_a_rust_keyword_becomes_a_raw_identifier() {
    // `type`, `match` and `ref` are ordinary column names in SQL and reserved
    // words in Rust. Emitting them bare would produce source that cannot parse.
    let pool = database(&["CREATE TABLE events (\
            id INTEGER PRIMARY KEY, \
            type TEXT NOT NULL, \
            match TEXT, \
            ref TEXT NOT NULL)"])
    .await;
    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();

    let expected = "\
#[derive(Entity)]
#[oxider(table = \"events\")]
pub struct Events {
    pub id: i64,
    pub r#type: String,
    pub r#match: Option<String>,
    pub r#ref: String,
}
";
    assert_eq!(valid_rust(&source), expected);
}

#[tokio::test]
async fn a_column_that_is_not_a_rust_identifier_is_renamed_and_mapped_back() {
    // A name with a space, a dash, or a leading digit cannot be a field name at
    // all, so the field is renamed and `#[oxider(column = ...)]` records the
    // real name. Without the attribute the generated struct would silently
    // query a column that does not exist.
    let pool = database(&["CREATE TABLE metrics (\
            id INTEGER PRIMARY KEY, \
            \"total-count\" INTEGER NOT NULL, \
            \"2fa enabled\" BOOLEAN NOT NULL)"])
    .await;
    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();

    let expected = "\
#[derive(Entity)]
#[oxider(table = \"metrics\")]
pub struct Metrics {
    pub id: i64,
    #[oxider(column = \"total-count\")]
    pub total_count: i64,
    #[oxider(column = \"2fa enabled\")]
    pub _2fa_enabled: bool,
}
";
    assert_eq!(valid_rust(&source), expected);
}

#[tokio::test]
async fn a_table_whose_name_is_not_a_rust_type_name_still_generates() {
    let pool = database(&["CREATE TABLE \"user-sessions\" (id INTEGER PRIMARY KEY)"]).await;
    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();

    let expected = "\
#[derive(Entity)]
#[oxider(table = \"user-sessions\")]
pub struct UserSessions {
    pub id: i64,
}
";
    assert_eq!(valid_rust(&source), expected);
}

#[tokio::test]
async fn declared_types_map_by_sqlite_affinity_and_nullability_follows_not_null() {
    let pool = database(&["CREATE TABLE things (\
            id INTEGER PRIMARY KEY, \
            code VARCHAR(32) NOT NULL, \
            note CLOB, \
            weight DOUBLE PRECISION NOT NULL, \
            ratio FLOAT, \
            flag BOOLEAN, \
            payload BLOB, \
            untyped, \
            amount DECIMAL(10, 2))"])
    .await;
    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();

    let expected = "\
#[derive(Entity)]
#[oxider(table = \"things\")]
pub struct Things {
    pub id: i64,
    pub code: String,
    pub note: Option<String>,
    pub weight: f64,
    pub ratio: Option<f64>,
    pub flag: Option<bool>,
    pub payload: Option<Vec<u8>>,
    pub untyped: Option<Vec<u8>>,
    pub amount: Option<String>,
}
";
    assert_eq!(valid_rust(&source), expected);
}

/// The items a source file declares, so a test can assert nothing extra was
/// smuggled in alongside the struct that was asked for.
fn item_names(source: &str) -> Vec<String> {
    syn::parse_file(source)
        .unwrap()
        .items
        .iter()
        .map(|item| match item {
            syn::Item::Struct(s) => format!("struct {}", s.ident),
            syn::Item::Fn(f) => format!("fn {}", f.sig.ident),
            _ => "other item".to_string(),
        })
        .collect()
}

#[tokio::test]
async fn a_column_name_cannot_inject_items_into_the_generated_source() {
    // A name crafted to close the `#[oxider(column = "...")]` literal, close
    // the struct, declare an item, and re-open a struct so the trailing field
    // and brace still parse. Before the names were escaped this produced a
    // valid Rust file containing `pub fn pwn`.
    let hostile = r#"a")] pub ok: i64 } pub fn pwn() -> i64 { 42 } pub struct Tail { x: i64, // "#;
    let quoted = hostile.replace('"', "\"\"");
    let pool = database(&[&format!(
        r#"CREATE TABLE t (id INTEGER PRIMARY KEY, "{quoted}" INTEGER)"#
    )])
    .await;

    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();

    assert_eq!(item_names(valid_rust(&source)), ["struct T"]);
    // The real column name survives, escaped, so the metamodel still queries it.
    assert!(source.contains(r#"#[oxider(column = "a\")] pub ok: i64 } pub fn pwn() -> i64 { 42 } pub struct Tail { x: i64, // ")]"#));
}

#[tokio::test]
async fn a_table_name_with_a_quote_is_introspected_and_escaped() {
    // The quote used to escape out of `PRAGMA table_info("...")`, so
    // introspection failed outright with a syntax error from SQLite.
    let pool = database(&[r#"CREATE TABLE "ev""il" (id INTEGER PRIMARY KEY)"#]).await;

    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();

    assert_eq!(
        valid_rust(&source),
        "\
#[derive(Entity)]
#[oxider(table = \"ev\\\"il\")]
pub struct EvIl {
    pub id: i64,
}
"
    );
    assert_eq!(item_names(&source), ["struct EvIl"]);
}

#[tokio::test]
async fn a_database_with_no_user_tables_generates_nothing() {
    let pool = database(&[]).await;
    let source = oxider_query_codegen::sqlite::generate_entities(&pool)
        .await
        .unwrap();
    assert_eq!(source, "");
    valid_rust(&source);
}

//! PostgreSQL introspection against a real server.
//!
//! Skipped unless `OXIDER_POSTGRES_URL` is set, so the default `cargo test` run
//! needs no server. Start one with `make pg-up`.
//!
//! Every assertion here is about generated *source text*, so each test creates
//! its own schema, generates from it, and drops it again. Two tests sharing the
//! public schema would see each other's tables in the output.

#![cfg(all(feature = "postgres", feature = "chrono"))]

use sqlx::{Executor, PgPool};

fn url() -> Option<String> {
    std::env::var("OXIDER_POSTGRES_URL").ok()
}

/// A private schema holding one test's tables, and a pool to generate from.
///
/// The name is the caller's, so two tests never collide even running at once -
/// which is why this suite needs no mutex, unlike the execution suites. That
/// only holds because the DDL really lands in the named schema, and `search_path`
/// is a property of one connection rather than of the pool: run over the pool,
/// each statement takes whichever connection is free and creates its table in
/// `public`, where every test then sees every other test's tables. So the whole
/// setup runs on one acquired connection.
async fn schema(name: &str, ddl: &[&str]) -> Option<PgPool> {
    let pool = PgPool::connect(&url()?).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        conn.execute(format!("DROP SCHEMA IF EXISTS {name} CASCADE").as_str())
            .await
            .unwrap();
        conn.execute(format!("CREATE SCHEMA {name}").as_str())
            .await
            .unwrap();
        conn.execute(format!("SET search_path TO {name}").as_str())
            .await
            .unwrap();
        for statement in ddl {
            conn.execute(*statement).await.unwrap();
        }
    }
    Some(pool)
}

/// Generate from one schema only, so the output is this test's tables and
/// nothing another test happens to have created at the same moment.
async fn generate(pool: &PgPool, schema: &str) -> String {
    oxider_query_codegen::postgres::generate_entities_in(pool, &[schema])
        .await
        .unwrap()
}

/// Everything generated has to parse as Rust. A struct that does not compile is
/// worse than no struct at all, so this runs on every output below.
#[track_caller]
fn parses_as_rust(source: &str) {
    syn::parse_file(source)
        .unwrap_or_else(|err| panic!("generated source does not parse: {err}\n{source}"));
}

macro_rules! pool_or_skip {
    ($pool:ident, $schema:literal, $ddl:expr) => {
        let Some($pool) = schema($schema, $ddl).await else {
            eprintln!("OXIDER_POSTGRES_URL is not set, skipping");
            return;
        };
    };
}

#[tokio::test]
async fn a_column_becomes_the_rust_type_postgres_says_it_is() {
    pool_or_skip!(
        pool,
        "cg_types",
        &["CREATE TABLE t (\
             a SMALLINT NOT NULL, b INTEGER NOT NULL, c BIGINT NOT NULL, \
             d REAL NOT NULL, e DOUBLE PRECISION NOT NULL, f BOOLEAN NOT NULL, \
             g TEXT NOT NULL, h VARCHAR(40) NOT NULL, i BYTEA NOT NULL, \
             j DATE NOT NULL, k TIME NOT NULL, l TIMESTAMP NOT NULL, \
             m TIMESTAMPTZ NOT NULL)"]
    );
    let source = generate(&pool, "cg_types").await;
    parses_as_rust(&source);

    for expected in [
        "pub a: i16,",
        "pub b: i32,",
        "pub c: i64,",
        "pub d: f32,",
        "pub e: f64,",
        "pub f: bool,",
        "pub g: String,",
        "pub h: String,",
        "pub i: Vec<u8>,",
        "pub j: chrono::NaiveDate,",
        "pub k: chrono::NaiveTime,",
        "pub l: chrono::NaiveDateTime,",
        "pub m: chrono::DateTime<chrono::Utc>,",
    ] {
        assert!(
            source.contains(expected),
            "expected `{expected}` in:\n{source}"
        );
    }
}

/// A length or precision changes nothing about which Rust type fits, and
/// `format_type` writes it into the name, so it has to be cut off before the
/// lookup rather than turning every sized column into an unmapped one.
#[tokio::test]
async fn a_length_or_precision_does_not_make_a_column_unmapped() {
    pool_or_skip!(
        pool,
        "cg_modifiers",
        &[
            "CREATE TABLE t (a VARCHAR(40) NOT NULL, b CHAR(3) NOT NULL, \
           c TIMESTAMP(3) WITH TIME ZONE NOT NULL)"
        ]
    );
    let source = generate(&pool, "cg_modifiers").await;
    parses_as_rust(&source);

    assert!(source.contains("pub a: String,"), "{source}");
    assert!(source.contains("pub b: String,"), "{source}");
    assert!(
        source.contains("pub c: chrono::DateTime<chrono::Utc>,"),
        "{source}"
    );
    assert!(
        !source.contains("no Rust mapping"),
        "none of these are unmapped:\n{source}"
    );
}

#[tokio::test]
async fn a_nullable_column_becomes_an_option() {
    pool_or_skip!(
        pool,
        "cg_null",
        &["CREATE TABLE t (id BIGINT PRIMARY KEY, required TEXT NOT NULL, optional TEXT)"]
    );
    let source = generate(&pool, "cg_null").await;
    parses_as_rust(&source);

    assert!(source.contains("pub id: i64,"), "{source}");
    assert!(source.contains("pub required: String,"), "{source}");
    assert!(source.contains("pub optional: Option<String>,"), "{source}");
}

/// The keys are the part a reader cannot recover from the Rust types, and the
/// part they need exactly when writing a join.
#[tokio::test]
async fn keys_are_reported_as_doc_comments() {
    pool_or_skip!(
        pool,
        "cg_keys",
        &[
            "CREATE TABLE users (id BIGINT PRIMARY KEY, name TEXT NOT NULL)",
            "CREATE TABLE posts (id BIGINT PRIMARY KEY, \
             author_id BIGINT NOT NULL REFERENCES users (id), body TEXT)",
        ]
    );
    let source = generate(&pool, "cg_keys").await;
    parses_as_rust(&source);

    assert!(source.contains("/// Primary key."), "{source}");
    assert!(
        source.contains("/// References `cg_keys.users`.`id`."),
        "the reference a join needs is named, qualified because the table is not \
         in `public`:\n{source}"
    );
}

/// A composite foreign key pairs each column with the column it points at, in
/// key order. Pairing by position is the whole reason the query unnests both
/// key arrays with ordinality rather than joining them separately.
#[tokio::test]
async fn a_composite_foreign_key_pairs_columns_by_position() {
    pool_or_skip!(
        pool,
        "cg_composite",
        &[
            "CREATE TABLE parent (a BIGINT NOT NULL, b TEXT NOT NULL, PRIMARY KEY (a, b))",
            "CREATE TABLE child (id BIGINT PRIMARY KEY, pa BIGINT NOT NULL, pb TEXT NOT NULL, \
             FOREIGN KEY (pa, pb) REFERENCES parent (a, b))",
        ]
    );
    let source = generate(&pool, "cg_composite").await;
    parses_as_rust(&source);

    assert!(
        source.contains("/// References `cg_composite.parent`.`a`."),
        "first column pairs with the first key column:\n{source}"
    );
    assert!(
        source.contains("/// References `cg_composite.parent`.`b`."),
        "second with the second, not with the first again:\n{source}"
    );
}

/// A type with no mapping still produces a field, so the file compiles, and
/// says what it really is, so nobody ships the guess by accident.
#[tokio::test]
async fn an_unmapped_type_falls_back_to_text_and_says_what_it_was() {
    pool_or_skip!(
        pool,
        "cg_unmapped",
        &["CREATE TABLE t (id BIGINT PRIMARY KEY, tags TEXT[] NOT NULL)"]
    );
    let source = generate(&pool, "cg_unmapped").await;
    parses_as_rust(&source);

    assert!(source.contains("pub tags: String,"), "{source}");
    assert!(
        source.contains("Column type is `text[]`"),
        "the real type is named:\n{source}"
    );
}

/// A table outside `public` carries the schema, because a query against it
/// needs the qualified name. A table inside `public` does not, because the
/// search path already resolves it and the attribute would be noise.
#[tokio::test]
async fn a_table_outside_public_carries_its_schema() {
    pool_or_skip!(
        pool,
        "cg_schema",
        &["CREATE TABLE thing (id BIGINT PRIMARY KEY)"]
    );
    let source = generate(&pool, "cg_schema").await;
    parses_as_rust(&source);

    assert!(
        source.contains("#[oxider(schema = \"cg_schema\")]"),
        "{source}"
    );
    assert!(source.contains("#[oxider(table = \"thing\")]"), "{source}");
}

/// A schema is data, not source. A table named so that it would close the
/// attribute and open a new item has to come back as a string literal and
/// nothing more.
#[tokio::test]
async fn a_hostile_table_name_stays_inside_its_string_literal() {
    pool_or_skip!(
        pool,
        "cg_hostile",
        // A doubled quote is how PostgreSQL spells a quote inside an identifier,
        // so the table is really named `t")] pub struct Evil; //`.
        &["CREATE TABLE \"t\"\")] pub struct Evil; //\" (id BIGINT PRIMARY KEY)"]
    );
    let source = generate(&pool, "cg_hostile").await;
    parses_as_rust(&source);

    // Searching the text for `pub struct Evil` proves nothing: it is in there,
    // inside the attribute's string literal, which is where it belongs. The
    // question is whether it is also an item, so count the items.
    let parsed = syn::parse_file(&source).unwrap();
    assert_eq!(
        parsed.items.len(),
        1,
        "the name closed the attribute and opened a second item:\n{source}"
    );
    assert!(
        source.contains(r#"#[oxider(table = "t\")] pub struct Evil; //")]"#),
        "the real name still comes back, escaped for a Rust literal:\n{source}"
    );
}

//! End-to-end scenarios against a real SQLite database.
//!
//! These cover the cases where rendering correctly is not enough and only
//! running the statement proves the behaviour: escaped `LIKE` patterns matching
//! what they should and nothing else, emulated null ordering sorting the way
//! the native clause would, an emulated aggregate filter counting the same rows
//! as the native one, a recursive CTE actually terminating, and a transaction
//! actually rolling back.
//!
//! There is no `.to_sql(&Sqlite)` at any call site: the `Db` handle renders each
//! query with its backend's dialect.

use oxider_query::prelude::*;
use oxider_query::{Caps, Operator, Sqlite, Template, Window};
use oxider_query_exec::{Result, SqliteDb};

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
    age: i64,
    active: bool,
    manager_id: Option<i64>,
}

#[derive(Entity, sqlx::FromRow, Debug, PartialEq)]
#[oxider(table = "posts")]
struct Post {
    id: i64,
    user_id: i64,
    title: String,
    views: i64,
}

/// One projected column, for queries that select a single expression.
#[derive(sqlx::FromRow, Debug, PartialEq)]
struct One<T> {
    value: T,
}

/// A fresh in-memory database with both tables.
async fn database() -> SqliteDb {
    let db = SqliteDb::connect("sqlite::memory:").await.unwrap();
    for statement in [
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT, \
         age INTEGER NOT NULL, active BOOLEAN NOT NULL, manager_id INTEGER)",
        "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, \
         title TEXT NOT NULL, views INTEGER NOT NULL)",
        "CREATE UNIQUE INDEX users_email ON users (email)",
    ] {
        sqlx::query(statement).execute(db.pool()).await.unwrap();
    }
    db
}

/// Insert one user through the builder.
async fn add_user(
    db: &SqliteDb,
    id: i64,
    name: &str,
    email: Option<&str>,
    age: i64,
    manager: Option<i64>,
) -> Result<u64> {
    db.execute(
        User::insert()
            .set(User::id, id)
            .set(User::name, name.to_string())
            .set(User::email, email.map(|value| value.to_string()))
            .set(User::age, age)
            .set(User::active, true)
            .set(User::manager_id, manager),
    )
    .await
}

#[tokio::test]
async fn an_insert_round_trips_through_a_select() {
    let db = database().await;
    let affected = add_user(&db, 1, "ada", Some("ada@example.com"), 36, None)
        .await
        .unwrap();
    assert_eq!(affected, 1);

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert_eq!(
        rows,
        vec![User {
            id: 1,
            name: "ada".into(),
            email: Some("ada@example.com".into()),
            age: 36,
            active: true,
            manager_id: None,
        }]
    );
}

#[tokio::test]
async fn a_multi_row_insert_writes_every_row() {
    let db = database().await;
    let affected = db
        .execute(
            Post::insert()
                .columns((Post::id, Post::user_id, Post::title, Post::views))
                .values((1, 1, "first", 10))
                .values((2, 1, "second", 20))
                .values((3, 2, "third", 30)),
        )
        .await
        .unwrap();
    assert_eq!(affected, 3);

    let count: One<i64> = db
        .fetch_one(Post::query().select(count_all().alias("value")))
        .await
        .unwrap();
    assert_eq!(count.value, 3);
}

#[tokio::test]
async fn an_escaped_like_matches_the_literal_text_and_nothing_else() {
    // The bug this escaping exists to prevent: without it, searching for "50%"
    // matches "500 units" too, because `%` is a wildcard.
    let db = database().await;
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

#[tokio::test]
async fn emulated_null_ordering_puts_the_nulls_where_the_native_clause_would() {
    let db = database().await;
    add_user(&db, 1, "with", Some("b@example.com"), 30, None)
        .await
        .unwrap();
    add_user(&db, 2, "without", None, 31, None).await.unwrap();
    add_user(&db, 3, "another", Some("a@example.com"), 32, None)
        .await
        .unwrap();

    // SQLite has no NULLS LAST, so this runs the emulated CASE sort key.
    let last: Vec<User> = db
        .fetch_all(User::query().order_by(User::email.asc().nulls_last()))
        .await
        .unwrap();
    assert_eq!(
        last.iter().map(|u| u.id).collect::<Vec<_>>(),
        vec![3, 1, 2],
        "nulls should sort last"
    );

    let first: Vec<User> = db
        .fetch_all(User::query().order_by(User::email.asc().nulls_first()))
        .await
        .unwrap();
    assert_eq!(
        first.iter().map(|u| u.id).collect::<Vec<_>>(),
        vec![2, 3, 1],
        "nulls should sort first"
    );
}

/// SQLite with `FILTER (WHERE ...)` switched off, so the same query renders
/// through the `CASE` emulation the engines without the clause receive.
///
/// Running both against the same database is the only way to show the
/// emulation counts the same rows.
#[derive(Debug, Default, Clone, Copy)]
struct SqliteWithoutFilter;

impl Dialect for SqliteWithoutFilter {
    fn name(&self) -> &'static str {
        "sqlite-without-filter"
    }
    fn quote_ident(&self, ident: &str) -> String {
        Sqlite.quote_ident(ident)
    }
    fn placeholder(&self, index: usize) -> String {
        Sqlite.placeholder(index)
    }
    fn template(&self, op: Operator) -> Option<Template> {
        Sqlite.template(op)
    }
    fn precedence(&self, op: Operator) -> i16 {
        Sqlite.precedence(op)
    }
    fn caps(&self) -> Caps {
        Caps {
            aggregate_filter: false,
            ..Sqlite.caps()
        }
    }
    fn cast_type(&self, kind: oxider_query::CastKind) -> &'static str {
        Sqlite.cast_type(kind)
    }
    fn bool_literal(&self, value: bool) -> &'static str {
        Sqlite.bool_literal(value)
    }
    fn unlimited_limit(&self) -> Option<&'static str> {
        Sqlite.unlimited_limit()
    }
}

#[tokio::test]
async fn an_emulated_aggregate_filter_counts_the_same_rows_as_the_native_one() {
    let db = database().await;
    db.execute(
        Post::insert()
            .columns((Post::id, Post::user_id, Post::title, Post::views))
            .values((1, 1, "a", 5))
            .values((2, 1, "b", 50))
            .values((3, 1, "c", 500)),
    )
    .await
    .unwrap();

    let popular =
        || Post::query().select(count_all().filter_where(Post::views.ge(50)).alias("value"));

    let native: One<i64> = db.fetch_one(popular()).await.unwrap();
    assert_eq!(native.value, 2);

    let emulated = popular().to_sql(&SqliteWithoutFilter).unwrap();
    assert!(
        emulated.sql.contains("CASE WHEN"),
        "expected the emulation, got: {}",
        emulated.sql
    );
    let value: (i64,) = sqlx::query_as(&emulated.sql)
        .bind(50i64)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        value.0, native.value,
        "the emulation must count the same rows"
    );
}

#[tokio::test]
async fn an_upsert_inserts_once_and_updates_afterwards() {
    let db = database().await;

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
async fn returning_hands_back_the_row_that_was_written() {
    let db = database().await;
    let inserted: One<i64> = db
        .fetch_one(
            User::insert()
                .set(User::id, 7)
                .set(User::name, "grace")
                .set(User::age, 45)
                .set(User::active, true)
                .returning(User::id.alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(inserted.value, 7);

    let deleted: One<String> = db
        .fetch_one(
            User::delete()
                .filter(User::id.eq(7))
                .returning(User::name.alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(deleted.value, "grace");
}

#[tokio::test]
async fn an_update_reads_the_column_it_writes() {
    let db = database().await;
    db.execute(
        Post::insert()
            .columns((Post::id, Post::user_id, Post::title, Post::views))
            .values((1, 1, "a", 10)),
    )
    .await
    .unwrap();

    db.execute(
        Post::update()
            .set(Post::views, Post::views.add(5))
            .filter(Post::id.eq(1)),
    )
    .await
    .unwrap();

    let row: Post = db
        .fetch_one(Post::query().filter(Post::id.eq(1)))
        .await
        .unwrap();
    assert_eq!(row.views, 15);
}

#[tokio::test]
async fn a_correlated_subquery_counts_per_outer_row() {
    let db = database().await;
    add_user(&db, 1, "ada", Some("a@example.com"), 36, None)
        .await
        .unwrap();
    add_user(&db, 2, "grace", Some("g@example.com"), 45, None)
        .await
        .unwrap();
    db.execute(
        Post::insert()
            .columns((Post::id, Post::user_id, Post::title, Post::views))
            .values((1, 1, "a", 1))
            .values((2, 1, "b", 2)),
    )
    .await
    .unwrap();

    let post_count = Post::query()
        .correlate::<User>()
        .filter(Post::user_id.eq(User::id))
        .scalar(count_all());

    #[derive(sqlx::FromRow, Debug, PartialEq)]
    struct Row {
        name: String,
        posts: i64,
    }

    let rows: Vec<Row> = db
        .fetch_all(
            User::query()
                .select((User::name, post_count.alias("posts")))
                .order_by(User::id.asc()),
        )
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![
            Row {
                name: "ada".into(),
                posts: 2
            },
            Row {
                name: "grace".into(),
                posts: 0
            },
        ]
    );
}

#[tokio::test]
async fn a_window_function_ranks_within_its_partition() {
    let db = database().await;
    db.execute(
        Post::insert()
            .columns((Post::id, Post::user_id, Post::title, Post::views))
            .values((1, 1, "low", 1))
            .values((2, 1, "high", 9))
            .values((3, 2, "only", 5)),
    )
    .await
    .unwrap();

    #[derive(sqlx::FromRow, Debug, PartialEq)]
    struct Ranked {
        title: String,
        position: i64,
    }

    let rows: Vec<Ranked> = db
        .fetch_all(
            Post::query()
                .select((
                    Post::title,
                    row_number()
                        .over(
                            Window::new()
                                .partition_by(Post::user_id)
                                .order_by(Post::views.desc()),
                        )
                        .alias("position"),
                ))
                .order_by(Post::id.asc()),
        )
        .await
        .unwrap();

    assert_eq!(
        rows,
        vec![
            Ranked {
                title: "low".into(),
                position: 2
            },
            Ranked {
                title: "high".into(),
                position: 1
            },
            Ranked {
                title: "only".into(),
                position: 1
            },
        ]
    );
}

#[tokio::test]
async fn a_recursive_cte_walks_the_management_chain() {
    let db = database().await;
    add_user(&db, 1, "root", Some("r@example.com"), 60, None)
        .await
        .unwrap();
    add_user(&db, 2, "middle", Some("m@example.com"), 45, Some(1))
        .await
        .unwrap();
    add_user(&db, 3, "leaf", Some("l@example.com"), 30, Some(2))
        .await
        .unwrap();

    let anchor = User::query()
        .filter(User::manager_id.is_null())
        .select((User::id, User::name));

    let step = User::query()
        .join_name_as("chain", "c", User::manager_id.eq(col::<i64>("c", "id")))
        .select((User::id, User::name));

    let rows: Vec<One<String>> = db
        .fetch_all(
            oxider_query::select_from_name("chain")
                .with("chain", anchor.union_all(step))
                .recursive()
                .select(col::<String>("chain", "name").alias("value"))
                .order_by(col::<String>("chain", "id").asc()),
        )
        .await
        .unwrap();

    assert_eq!(
        rows.iter().map(|r| r.value.as_str()).collect::<Vec<_>>(),
        vec!["root", "middle", "leaf"]
    );
}

#[tokio::test]
async fn a_transaction_that_returns_an_error_rolls_back() {
    let db = database().await;
    add_user(&db, 1, "ada", Some("a@example.com"), 36, None)
        .await
        .unwrap();

    let outcome: Result<()> = db
        .transaction(async |tx| {
            tx.execute(User::update().set(User::age, 99).filter(User::id.eq(1)))
                .await?;
            // Violating the unique index aborts the transaction.
            tx.execute(
                User::insert()
                    .set(User::id, 2)
                    .set(User::name, "clash")
                    .set(User::email, "a@example.com")
                    .set(User::age, 1)
                    .set(User::active, true),
            )
            .await?;
            Ok(())
        })
        .await;
    // The caller's own error comes back, not whatever the rollback reported.
    match outcome {
        Err(oxider_query_exec::Error::Database(err)) => {
            assert!(
                err.to_string().contains("UNIQUE constraint failed"),
                "the statement's error must survive the rollback, got: {err}"
            );
        }
        other => panic!("expected the conflicting insert to fail, got {other:?}"),
    }

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].age, 36, "the update must have rolled back");
}

#[tokio::test]
async fn statements_the_engine_cannot_run_are_refused_before_reaching_it() {
    let db = database().await;

    // SQLite has no multi-table delete, so `USING` is refused here rather than
    // failing at the database with a syntax error pointing at the wrong thing.
    let outcome = db
        .execute(User::delete().using(Post::table()).filter(User::id.eq(1)))
        .await;
    match outcome {
        Err(oxider_query_exec::Error::Render(err)) => {
            assert!(err.to_string().contains("DELETE ... USING"), "{err}");
        }
        other => panic!("expected a render error, got {other:?}"),
    }

    // An UPDATE assigning nothing would render `SET` with nothing after it.
    let outcome = db.execute(User::update().filter(User::id.eq(1))).await;
    match outcome {
        Err(oxider_query_exec::Error::Render(err)) => {
            assert!(err.to_string().contains("at least one assignment"), "{err}");
        }
        other => panic!("expected a render error, got {other:?}"),
    }
}

#[tokio::test]
async fn a_transaction_that_succeeds_commits_every_statement() {
    let db = database().await;
    db.transaction(async |tx| {
        tx.execute(
            User::insert()
                .set(User::id, 1)
                .set(User::name, "ada")
                .set(User::age, 36)
                .set(User::active, true),
        )
        .await?;
        tx.execute(User::update().set(User::age, 37).filter(User::id.eq(1)))
            .await?;
        Ok::<(), oxider_query_exec::Error>(())
    })
    .await
    .unwrap();

    let rows: Vec<User> = db.fetch_all(User::query()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].age, 37);
}

#[tokio::test]
async fn a_query_the_dialect_cannot_express_fails_before_reaching_the_database() {
    let db = database().await;
    // SQLite has no row locking, so this is refused at render time and the
    // error names the dialect rather than surfacing as a SQL syntax error.
    let outcome: Result<Vec<User>> = db.fetch_all(User::query().for_update()).await;
    match outcome {
        Err(oxider_query_exec::Error::Render(err)) => {
            assert!(err.to_string().contains("row locking"), "{err}");
        }
        other => panic!("expected a render error, got {other:?}"),
    }
}

#[tokio::test]
async fn paging_returns_disjoint_pages_in_a_stable_order() {
    let db = database().await;
    for id in 1..=5 {
        add_user(&db, id, &format!("user{id}"), None, 20 + id, None)
            .await
            .unwrap();
    }

    let page = |n: u64| User::query().order_by(User::id.asc()).page(n, 2);

    let first: Vec<User> = db.fetch_all(page(0)).await.unwrap();
    let second: Vec<User> = db.fetch_all(page(1)).await.unwrap();
    let third: Vec<User> = db.fetch_all(page(2)).await.unwrap();

    assert_eq!(first.iter().map(|u| u.id).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(second.iter().map(|u| u.id).collect::<Vec<_>>(), vec![3, 4]);
    assert_eq!(third.iter().map(|u| u.id).collect::<Vec<_>>(), vec![5]);
}

#[tokio::test]
async fn an_offset_with_no_limit_still_skips_rows() {
    let db = database().await;
    for id in 1..=3 {
        add_user(&db, id, &format!("user{id}"), None, 20 + id, None)
            .await
            .unwrap();
    }

    // SQLite needs a LIMIT before OFFSET; the dialect supplies `-1`.
    let rows: Vec<User> = db
        .fetch_all(User::query().order_by(User::id.asc()).offset(1))
        .await
        .unwrap();
    assert_eq!(rows.iter().map(|u| u.id).collect::<Vec<_>>(), vec![2, 3]);
}

#[tokio::test]
async fn an_offset_position_search_agrees_with_the_native_function_it_emulates() {
    // SQLite has no three-argument `INSTR`, so searching from an offset is
    // emulated by searching the remainder and shifting the result back. The
    // shift is only correct when a miss stays a miss, which is what the
    // surrounding `CASE` is for: without it, an absent needle reported
    // `start - 1` instead of 0, which reads as a match near the front.
    let db = database().await;
    add_user(&db, 1, "abcabc", None, 30, None).await.unwrap();

    let found: One<i64> = db
        .fetch_one(
            User::query()
                .filter(User::id.eq(1))
                .select(User::name.index_of_from("bc", 3).alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(found.value, 5, "the second `bc` starts at position 5");

    let missing: One<i64> = db
        .fetch_one(
            User::query()
                .filter(User::id.eq(1))
                .select(User::name.index_of_from("zz", 3).alias("value")),
        )
        .await
        .unwrap();
    assert_eq!(missing.value, 0, "an absent needle is 0, not an offset");
}

//! Positional projections and the group-by fold, against a real SQLite
//! database.
//!
//! Running these matters more than rendering them. `#[derive(Projection)]`
//! promises that field *n* reads column *n* of its span, which only a real row
//! can confirm, and `group_children` promises an order that only a real
//! `ORDER BY` can be checked against.

#![cfg(feature = "sqlite")]

use oxider_query::prelude::*;
use oxider_query_exec::{group_children, Result, SqliteDb};

#[derive(Entity, Projection, Debug, PartialEq)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i64,
}

#[derive(Entity, Projection, Debug, PartialEq)]
#[oxider(table = "orders")]
struct Order {
    id: i64,
    user_id: i64,
    total: i64,
}

/// The same columns as [`User`], declared in a different order.
///
/// Nothing but the order changes, so a by-name mapping would fill it exactly
/// like `User`. A positional one cannot, which is what makes this the test that
/// the derive is positional at all.
#[derive(Projection, Debug, PartialEq)]
struct UserReversed {
    age: i64,
    name: String,
    id: i64,
}

/// A projection with a field that is not a column.
#[derive(Projection, Debug, PartialEq)]
struct UserWithNote {
    id: i64,
    name: String,
    #[oxider(skip)]
    note: String,
}

async fn database() -> SqliteDb {
    let db = SqliteDb::connect("sqlite::memory:").await.unwrap();
    for statement in [
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER NOT NULL)",
        "CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, \
         total INTEGER NOT NULL)",
    ] {
        sqlx::query(statement).execute(db.pool()).await.unwrap();
    }
    db
}

async fn add_user(db: &SqliteDb, id: i64, name: &str, age: i64) -> Result<u64> {
    db.execute(
        User::insert()
            .set(User::id, id)
            .set(User::name, name.to_string())
            .set(User::age, age),
    )
    .await
}

async fn add_order(db: &SqliteDb, id: i64, user_id: i64, total: i64) -> Result<u64> {
    db.execute(
        Order::insert()
            .set(Order::id, id)
            .set(Order::user_id, user_id)
            .set(Order::total, total),
    )
    .await
}

#[tokio::test]
async fn a_projection_reads_the_select_list_in_order() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();

    let found: Vec<User> = db
        .fetch_all_projected(User::query().select((User::id, User::name, User::age)))
        .await
        .unwrap();
    assert_eq!(
        found,
        vec![User {
            id: 1,
            name: "ada".into(),
            age: 36
        }]
    );
}

/// The point of positional mapping: the struct follows the `select` list, so
/// selecting the same columns in another order fills another struct. A by-name
/// mapping would return the same values for both.
#[tokio::test]
async fn the_order_of_the_select_list_decides_which_field_gets_which_column() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();

    let reversed: Vec<UserReversed> = db
        .fetch_all_projected(User::query().select((User::age, User::name, User::id)))
        .await
        .unwrap();
    assert_eq!(
        reversed,
        vec![UserReversed {
            age: 36,
            name: "ada".into(),
            id: 1
        }]
    );
}

/// No aliases anywhere in this query. A `FromRow` mapping would need one per
/// expression to find its field.
#[tokio::test]
async fn a_projection_needs_no_aliases() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_order(&db, 10, 1, 500).await.unwrap();

    #[derive(Projection, Debug, PartialEq)]
    struct NameAndTotal {
        name: String,
        total: i64,
    }

    let rows: Vec<NameAndTotal> = db
        .fetch_all_projected(
            User::query()
                .inner_join(Order::table(), Order::user_id.eq(User::id))
                .select((User::name, Order::total.sum())),
        )
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![NameAndTotal {
            name: "ada".into(),
            total: 500
        }]
    );
}

#[tokio::test]
async fn a_skipped_field_consumes_no_column_and_defaults() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();

    let rows: Vec<UserWithNote> = db
        .fetch_all_projected(User::query().select((User::id, User::name)))
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![UserWithNote {
            id: 1,
            name: "ada".into(),
            note: String::new()
        }]
    );
}

/// A tuple splits one flat row into two structs, with the second starting where
/// the first ended. The widths are added up at compile time, not read from the
/// row.
#[tokio::test]
async fn a_tuple_of_projections_splits_one_flat_join() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_order(&db, 10, 1, 500).await.unwrap();

    let rows: Vec<(User, Order)> = db
        .fetch_all_projected(
            User::query()
                .inner_join(Order::table(), Order::user_id.eq(User::id))
                .select((
                    User::id,
                    User::name,
                    User::age,
                    Order::id,
                    Order::user_id,
                    Order::total,
                )),
        )
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![(
            User {
                id: 1,
                name: "ada".into(),
                age: 36
            },
            Order {
                id: 10,
                user_id: 1,
                total: 500
            }
        )]
    );
}

/// The outer half of a `LEFT JOIN` is `None` only when the whole span is NULL.
#[tokio::test]
async fn an_optional_projection_is_none_when_its_whole_span_is_null() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_user(&db, 2, "grace", 45).await.unwrap();
    add_order(&db, 10, 1, 500).await.unwrap();

    let rows: Vec<(User, Option<Order>)> = db
        .fetch_all_projected(
            User::query()
                .left_join(Order::table(), Order::user_id.eq(User::id))
                .select((
                    User::id,
                    User::name,
                    User::age,
                    Order::id,
                    Order::user_id,
                    Order::total,
                ))
                .order_by(User::id.asc()),
        )
        .await
        .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1.as_ref().map(|order| order.id), Some(10));
    assert_eq!(rows[1].1, None, "grace has no orders");
}

/// The whole point of the two features together: a one-to-many join comes back
/// flat and leaves as a tree, without a second query per parent.
#[tokio::test]
async fn group_children_folds_a_one_to_many_join() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_user(&db, 2, "grace", 45).await.unwrap();
    add_user(&db, 3, "alan", 41).await.unwrap();
    add_order(&db, 10, 1, 500).await.unwrap();
    add_order(&db, 11, 1, 250).await.unwrap();
    add_order(&db, 12, 2, 900).await.unwrap();

    let rows: Vec<(User, Option<Order>)> = db
        .fetch_all_projected(
            User::query()
                .left_join(Order::table(), Order::user_id.eq(User::id))
                .select((
                    User::id,
                    User::name,
                    User::age,
                    Order::id,
                    Order::user_id,
                    Order::total,
                ))
                .order_by(User::id.asc())
                .order_by(Order::id.asc()),
        )
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        4,
        "ada twice, grace once, alan once with no order"
    );

    let tree = group_children(rows, |user| user.id);
    assert_eq!(
        tree.iter()
            .map(|(user, orders)| (user.id, orders.iter().map(|o| o.id).collect::<Vec<_>>()))
            .collect::<Vec<_>>(),
        vec![(1, vec![10, 11]), (2, vec![12]), (3, vec![])],
        "the ORDER BY survives the fold, and a parent with no children keeps an empty list"
    );
}

/// A projection wider than the query's own list has nowhere to read its last
/// field from. Nothing catches that at compile time - `Select` erases its
/// projection as soon as it is built - so this pins the runtime failure as a
/// clear column error rather than a wrong value.
#[tokio::test]
async fn a_projection_wider_than_the_select_list_fails_on_the_column_it_lacks() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();

    let outcome: Result<Vec<User>> = db
        .fetch_all_projected(User::query().select((User::id, User::name)))
        .await;
    match outcome {
        Err(oxider_query_exec::Error::Database(err)) => {
            assert!(
                err.to_string().to_lowercase().contains("index"),
                "expected a column-index error, got: {err}"
            );
        }
        other => panic!("expected a database error, got {other:?}"),
    }
}

/// A count has to survive `DISTINCT`, which is exactly where swapping the
/// projection for `COUNT(*)` instead of wrapping the query gives the wrong
/// answer: three rows in, two distinct ages.
#[tokio::test]
async fn a_count_wrapping_a_distinct_query_counts_distinct_rows() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();
    add_user(&db, 2, "grace", 36).await.unwrap();
    add_user(&db, 3, "alan", 41).await.unwrap();

    let distinct_ages = db
        .fetch_count(User::query().distinct().select(User::age))
        .await
        .unwrap();
    assert_eq!(distinct_ages, 2);

    let everyone = db.fetch_count(User::query()).await.unwrap();
    assert_eq!(everyone, 3);
}

/// The total must describe the whole query, not the page taken from it.
#[tokio::test]
async fn a_page_carries_the_total_of_the_unpaged_query() {
    let db = database().await;
    for id in 1..=7 {
        add_user(&db, id, &format!("user{id}"), 20 + id)
            .await
            .unwrap();
    }

    let page = db
        .fetch_page_projected::<User, _, _, _>(User::query().order_by(User::id.asc()), 1, 3)
        .await
        .unwrap();

    assert_eq!(
        page.items.iter().map(|u| u.id).collect::<Vec<_>>(),
        vec![4, 5, 6],
        "the second page of three"
    );
    assert_eq!(page.total, 7, "the total is of the whole query");
    assert_eq!(page.total_pages(), 3, "seven rows in pages of three");
    assert!(page.has_next());
    assert!(page.has_previous());
}

/// A page past the end still reports the total. This is what a windowed
/// `COUNT(*) OVER ()` cannot do: with no rows to attach the count to, it
/// returns nothing at all.
#[tokio::test]
async fn a_page_past_the_end_is_empty_but_still_knows_the_total() {
    let db = database().await;
    add_user(&db, 1, "ada", 36).await.unwrap();

    let page = db
        .fetch_page_projected::<User, _, _, _>(User::query(), 9, 10)
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.total, 1);
    assert_eq!(page.total_pages(), 1);
    assert!(!page.has_next());
}

/// A filter has to reach the count as well as the page, or the two describe
/// different queries.
#[tokio::test]
async fn a_filter_narrows_the_total_as_well_as_the_page() {
    let db = database().await;
    for id in 1..=6 {
        add_user(&db, id, &format!("user{id}"), 20 + id)
            .await
            .unwrap();
    }

    let page = db
        .fetch_page_projected::<User, _, _, _>(
            User::query()
                .filter(User::age.ge(24))
                .order_by(User::id.asc()),
            0,
            2,
        )
        .await
        .unwrap();
    assert_eq!(page.total, 3, "ages 24, 25 and 26");
    assert_eq!(page.items.len(), 2);
}

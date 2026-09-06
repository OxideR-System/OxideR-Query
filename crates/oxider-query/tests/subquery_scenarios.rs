//! Subquery scenarios: membership, existence, scalar subqueries in the
//! projection, correlation and quantified comparisons.
//!
//! Correlation is the interesting case. A subquery that references the outer
//! query must declare it with `correlate`, which puts the outer entity in the
//! subquery's scope and, at the same time, makes the resulting expression carry
//! that entity - so embedding it anywhere the outer table is not in scope is a
//! compile error rather than a runtime one.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres};

#[test]
fn an_in_subquery_takes_the_place_of_a_value_list() {
    let busy_authors = Post::query()
        .filter(Post::views.gt(1000))
        .scalar(Post::user_id);
    let query = User::query().filter(User::id.in_subquery(busy_authors));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."id" IN (SELECT "posts"."user_id" FROM "posts" WHERE "posts"."views" > $1)"#,
        &[int(1000)],
    );
}

#[test]
fn a_not_in_subquery_excludes_the_matching_rows() {
    let authors = Post::query().scalar(Post::user_id);
    let query = User::query().filter(User::id.not_in_subquery(authors));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."id" NOT IN (SELECT "posts"."user_id" FROM "posts")"#,
    );
}

#[test]
fn exists_ignores_the_projection_and_only_asks_whether_a_row_exists() {
    let has_posts = Post::query()
        .correlate::<User>()
        .filter(Post::user_id.eq(User::id));
    let query = User::query().filter(exists(has_posts));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" WHERE EXISTS (SELECT * FROM "posts" WHERE "posts"."user_id" = "users"."id")"#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT * FROM `users` WHERE EXISTS (SELECT * FROM `posts` WHERE `posts`.`user_id` = `users`.`id`)",
    );
}

#[test]
fn not_exists_finds_the_rows_with_no_match() {
    let has_orders = Order::query()
        .correlate::<User>()
        .filter(Order::user_id.eq(User::id));
    let query = User::query()
        .filter(not_exists(has_orders))
        .select(User::name);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name" FROM "users" WHERE NOT EXISTS (SELECT * FROM "orders" WHERE "orders"."user_id" = "users"."id")"#,
    );
}

#[test]
fn a_correlated_scalar_subquery_can_sit_in_the_projection() {
    let post_count = Post::query()
        .correlate::<User>()
        .filter(Post::user_id.eq(User::id))
        .scalar(count_all());
    let query = User::query().select((User::name, post_count.alias("posts")));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name", (SELECT COUNT(*) FROM "posts" WHERE "posts"."user_id" = "users"."id") AS "posts" FROM "users""#,
    );
}

#[test]
fn a_scalar_subquery_compares_against_a_column() {
    let average = Order::query().scalar(Order::total.avg());
    let query = Order::query()
        .filter(Order::total.gt(average))
        .select(Order::id);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "orders"."id" FROM "orders" WHERE "orders"."total" > (SELECT AVG("orders"."total") FROM "orders")"#,
    );
}

#[test]
fn any_holds_when_the_comparison_matches_a_single_returned_row() {
    let cheap = OrderItem::query()
        .filter(OrderItem::quantity.eq(1))
        .scalar(OrderItem::price);
    let query = OrderItem::query().filter(OrderItem::price.gt(cheap.any()));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "order_items" WHERE "order_items"."price" > ANY (SELECT "order_items"."price" FROM "order_items" WHERE "order_items"."quantity" = $1)"#,
        &[int(1)],
    );
}

#[test]
fn all_holds_only_when_the_comparison_matches_every_returned_row() {
    let cheap = OrderItem::query()
        .filter(OrderItem::quantity.eq(1))
        .scalar(OrderItem::price);
    let query = OrderItem::query().filter(OrderItem::price.ge(cheap.all()));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "order_items" WHERE "order_items"."price" >= ALL (SELECT "order_items"."price" FROM "order_items" WHERE "order_items"."quantity" = $1)"#,
        &[int(1)],
    );
}

#[test]
fn a_correlated_subquery_may_reach_two_levels_out() {
    // The item subquery is free in both Order and User, so both have to be in
    // scope where it is used.
    let item_total = OrderItem::query()
        .correlate::<Order>()
        .filter(OrderItem::order_id.eq(Order::id))
        .scalar(OrderItem::price.sum());

    let query = User::query()
        .inner_join(Order::table(), Order::user_id.eq(User::id))
        .select((User::name, item_total.alias("items_total")));

    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name", (SELECT SUM("order_items"."price") FROM "order_items" WHERE "order_items"."order_id" = "orders"."id") AS "items_total" FROM "users" INNER JOIN "orders" ON "orders"."user_id" = "users"."id""#,
    );
}

#[test]
fn placeholders_keep_counting_across_a_nested_subquery() {
    let recent = Post::query()
        .filter(Post::views.gt(10))
        .scalar(Post::user_id);
    let query = User::query()
        .filter(User::active.eq(true))
        .filter(User::id.in_subquery(recent))
        .filter(User::age.ge(21));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."active" = $1 AND "users"."id" IN (SELECT "posts"."user_id" FROM "posts" WHERE "posts"."views" > $2) AND "users"."age" >= $3"#,
        &[flag(true), int(10), int(21)],
    );
}

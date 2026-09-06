//! Aggregate scenarios: grouping, having, distinct, filtered and ordered
//! aggregates.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite};

#[test]
fn counting_rows_per_group_needs_no_column() {
    let query = Post::query()
        .select((Post::user_id, count_all()))
        .group_by(Post::user_id);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "posts"."user_id", COUNT(*) FROM "posts" GROUP BY "posts"."user_id""#,
    );
}

#[test]
fn having_filters_groups_after_aggregation() {
    let query = Post::query()
        .select((Post::user_id, count_all()))
        .group_by(Post::user_id)
        .having(count_all().gt(5))
        .order_by(Post::user_id.asc());
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "posts"."user_id", COUNT(*) FROM "posts" GROUP BY "posts"."user_id" HAVING COUNT(*) > $1 ORDER BY "posts"."user_id" ASC"#,
        &[int(5)],
    );
}

#[test]
fn the_arithmetic_aggregates_keep_or_widen_the_operand_type() {
    let query = Order::query()
        .select((Order::total.sum(), Order::total.avg(), Order::total.max()))
        .filter(Order::status.eq("paid"));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT SUM("orders"."total"), AVG("orders"."total"), MAX("orders"."total") FROM "orders" WHERE "orders"."status" = $1"#,
        &[text("paid")],
    );
}

#[test]
fn distinct_lands_inside_the_aggregate_call() {
    let query = Order::query().select(Order::user_id.count_distinct());
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT COUNT(DISTINCT "orders"."user_id") FROM "orders""#,
    );
}

#[test]
fn a_filtered_aggregate_is_native_on_postgres_and_a_case_on_mysql() {
    let paid = count_all().filter_where(Order::status.eq("paid"));
    let query = Order::query()
        .select((Order::user_id, paid))
        .group_by(Order::user_id);

    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT "orders"."user_id", COUNT(*) FILTER (WHERE "orders"."status" = $1) FROM "orders" GROUP BY "orders"."user_id""#,
        &[text("paid")],
    );
    // The emulation counts the same rows: COUNT ignores the NULLs the CASE
    // produces for non-matching rows.
    assert_sql(
        query,
        &MySql,
        "SELECT `orders`.`user_id`, COUNT(CASE WHEN `orders`.`status` = ? THEN 1 END) FROM `orders` \
         GROUP BY `orders`.`user_id`",
        &[text("paid")],
    );
}

#[test]
fn a_filter_on_a_valued_aggregate_wraps_the_aggregated_expression() {
    let paid_total = Order::total.sum().filter_where(Order::status.eq("paid"));
    let query = Order::query().select(paid_total);
    assert_sql(
        query,
        &MySql,
        "SELECT SUM(CASE WHEN `orders`.`status` = ? THEN `orders`.`total` END) FROM `orders`",
        &[text("paid")],
    );
}

#[test]
fn string_aggregation_has_a_different_name_and_shape_on_every_engine() {
    let query = Post::query()
        .select(group_concat(Post::title, ", "))
        .group_by(Post::user_id);
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT STRING_AGG("posts"."title", $1) FROM "posts" GROUP BY "posts"."user_id""#,
        &[text(", ")],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT GROUP_CONCAT(`posts`.`title` SEPARATOR ?) FROM `posts` GROUP BY `posts`.`user_id`",
        &[text(", ")],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT GROUP_CONCAT("posts"."title", ?) FROM "posts" GROUP BY "posts"."user_id""#,
        &[text(", ")],
    );
}

#[test]
fn an_ordered_aggregate_sorts_inside_the_call() {
    let query = Post::query()
        .select(group_concat(Post::title, ", ").order_by(Post::title.asc()))
        .group_by(Post::user_id);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT STRING_AGG("posts"."title" ORDER BY "posts"."title" ASC, $1) FROM "posts" GROUP BY "posts"."user_id""#,
        &[text(", ")],
    );
}

#[test]
fn boolean_aggregation_is_emulated_with_min_and_max() {
    let query = User::query().select(bool_and(User::active));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT BOOL_AND("users"."active") FROM "users""#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT (MIN(`users`.`active`) <> 0) FROM `users`",
    );
}

#[test]
fn a_statistical_aggregate_sqlite_lacks_is_refused_rather_than_faked() {
    let query = Department::query().select(Department::budget.std_dev());
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT STDDEV("departments"."budget") FROM "departments""#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT STDDEV_SAMP(`departments`.`budget`) FROM `departments`",
    );
    assert_rejected(query, &Sqlite, "StdDev");
}

#[test]
fn an_aggregate_over_a_join_groups_by_the_outer_key() {
    let query = User::query()
        .left_join(Order::table(), Order::user_id.eq(User::id))
        .select((User::name, Order::total.sum().alias("lifetime_value")))
        .group_by(User::name)
        .having(Order::total.sum().gt(1000.0))
        .order_by(User::name.asc());
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "users"."name", SUM("orders"."total") AS "lifetime_value" FROM "users" LEFT JOIN "orders" ON "orders"."user_id" = "users"."id" GROUP BY "users"."name" HAVING SUM("orders"."total") > $1 ORDER BY "users"."name" ASC"#,
        &[real(1000.0)],
    );
}

#[test]
fn grouping_accepts_several_keys_at_once() {
    let query = Order::query()
        .select((Order::user_id, Order::status, count_all()))
        .group_by((Order::user_id, Order::status));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "orders"."user_id", "orders"."status", COUNT(*) FROM "orders" GROUP BY "orders"."user_id", "orders"."status""#,
    );
}

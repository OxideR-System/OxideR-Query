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

// -- Ordered-set aggregates -------------------------------------------------
//
// A percentile sorts the group rather than the argument, so its ORDER BY sits
// after the call rather than inside it. PostgreSQL is the only built-in dialect
// with the clause, and the tests below pin both halves of that: what it renders
// where it exists, and that it is refused rather than approximated where it
// does not.

#[test]
fn a_continuous_percentile_sorts_the_group_after_the_call() {
    let query = Order::query()
        .select(
            percentile_cont(0.5)
                .within_group(Order::total)
                .alias("median"),
        )
        .group_by(Order::user_id);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT PERCENTILE_CONT($1) WITHIN GROUP (ORDER BY "orders"."total" ASC) AS "median" FROM "orders" GROUP BY "orders"."user_id""#,
        &[real(0.5)],
    );
}

/// The fraction is bound, not spliced. It reaches SQL as a parameter like any
/// other number, so the same prepared statement serves every percentile.
#[test]
fn the_fraction_binds_as_a_parameter() {
    let query = Department::query().select(percentile_disc(0.9).within_group(Department::budget));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT PERCENTILE_DISC($1) WITHIN GROUP (ORDER BY "departments"."budget" ASC) FROM "departments""#,
        &[real(0.9)],
    );
}

#[test]
fn a_percentile_can_sort_the_group_descending() {
    let query = Order::query().select(percentile_cont(0.1).within_group_desc(Order::total));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT PERCENTILE_CONT($1) WITHIN GROUP (ORDER BY "orders"."total" DESC) FROM "orders""#,
        &[real(0.1)],
    );
}

/// `WITHIN GROUP` comes before `FILTER`, which is the order PostgreSQL parses
/// and the reverse of how the two builder methods read.
#[test]
fn a_filtered_percentile_puts_within_group_first() {
    let query = Order::query().select(
        percentile_cont(0.5)
            .within_group(Order::total)
            .filter_where(Order::status.eq("paid")),
    );
    assert_sql(
        query,
        &Postgres,
        r#"SELECT PERCENTILE_CONT($1) WITHIN GROUP (ORDER BY "orders"."total" ASC) FILTER (WHERE "orders"."status" = $2) FROM "orders""#,
        &[real(0.5), text("paid")],
    );
}

/// A percentile is an aggregate, so it is legal in HAVING like any other.
#[test]
fn a_percentile_is_an_aggregate_in_having() {
    let query = Order::query()
        .select(Order::user_id)
        .group_by(Order::user_id)
        .having(percentile_cont(0.5).within_group(Order::total).gt(50.0));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "orders"."user_id" FROM "orders" GROUP BY "orders"."user_id" HAVING PERCENTILE_CONT($1) WITHIN GROUP (ORDER BY "orders"."total" ASC) > $2"#,
        &[real(0.5), real(50.0)],
    );
}

/// Neither engine has an ordered-set aggregate, and neither has anything to
/// stand in for one: a median is a property of the sorted group, so no
/// expression over a single row reproduces it. Both refuse while rendering.
#[test]
fn the_engines_without_within_group_refuse_rather_than_approximate() {
    let query = Order::query().select(percentile_cont(0.5).within_group(Order::total));
    assert_rejected(query.clone(), &MySql, "WITHIN GROUP");
    assert_rejected(query, &Sqlite, "WITHIN GROUP");
}

/// A discrete percentile returns a value the group actually holds, so it keeps
/// the sorted column's type rather than widening to a float. This compiles only
/// because `placed_on` is a date going in and a date coming out.
#[test]
fn a_discrete_percentile_keeps_the_sorted_columns_type() {
    let query = Order::query().select(
        percentile_disc(0.5)
            .within_group(Order::placed_on)
            .alias("median_day"),
    );
    assert_sql(
        query,
        &Postgres,
        r#"SELECT PERCENTILE_DISC($1) WITHIN GROUP (ORDER BY "orders"."placed_on" ASC) AS "median_day" FROM "orders""#,
        &[real(0.5)],
    );
}

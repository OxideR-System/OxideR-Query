//! Window-function scenarios: partitioning, ordering, frames and named windows.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite, Window};

#[test]
fn a_ranking_function_partitions_and_orders_within_the_partition() {
    let rank = row_number().over(
        Window::new()
            .partition_by(Post::user_id)
            .order_by(Post::views.desc()),
    );
    let query = Post::query().select((Post::title, rank.alias("position")));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "posts"."title", ROW_NUMBER() OVER (PARTITION BY "posts"."user_id" ORDER BY "posts"."views" DESC) AS "position" FROM "posts""#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT `posts`.`title`, ROW_NUMBER() OVER (PARTITION BY `posts`.`user_id` \
         ORDER BY `posts`.`views` DESC) AS `position` FROM `posts`",
    );
}

#[test]
fn an_aggregate_becomes_a_running_total_over_a_frame() {
    let running = Order::total.sum().over(
        Window::new()
            .partition_by(Order::user_id)
            .order_by(Order::placed_on.asc())
            .rows(FrameBound::UnboundedPreceding, Some(FrameBound::CurrentRow)),
    );
    let query = Order::query().select((Order::id, running.alias("running_total")));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "orders"."id", SUM("orders"."total") OVER (PARTITION BY "orders"."user_id" ORDER BY "orders"."placed_on" ASC ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS "running_total" FROM "orders""#,
    );
}

#[test]
fn a_one_sided_frame_omits_the_between() {
    let recent = Post::views.sum().over(
        Window::new()
            .order_by(Post::published_at.asc())
            .rows(FrameBound::Preceding(2), None),
    );
    let query = Post::query().select(recent);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT SUM("posts"."views") OVER (ORDER BY "posts"."published_at" ASC ROWS 2 PRECEDING) FROM "posts""#,
    );
}

#[test]
fn an_exclusion_clause_follows_the_frame() {
    let peers = count_all().over(
        Window::new()
            .order_by(Post::views.asc())
            .range(
                FrameBound::UnboundedPreceding,
                Some(FrameBound::UnboundedFollowing),
            )
            .exclude(FrameExclusion::CurrentRow),
    );
    let query = Post::query().select(peers);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT COUNT(*) OVER (ORDER BY "posts"."views" ASC RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING EXCLUDE CURRENT ROW) FROM "posts""#,
    );
}

#[test]
fn a_named_window_is_declared_once_and_referenced_by_name() {
    let query = Post::query()
        .window(
            "by_author",
            Window::new()
                .partition_by(Post::user_id)
                .order_by(Post::views.desc()),
        )
        .select((
            Post::title,
            row_number().over_named("by_author").alias("position"),
            Post::views
                .sum()
                .over_named("by_author")
                .alias("author_views"),
        ));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "posts"."title", ROW_NUMBER() OVER "by_author" AS "position", SUM("posts"."views") OVER "by_author" AS "author_views" FROM "posts" WINDOW "by_author" AS (PARTITION BY "posts"."user_id" ORDER BY "posts"."views" DESC)"#,
    );
}

#[test]
fn lag_reaches_back_a_fixed_number_of_rows() {
    let previous = lag(Order::total, 1).over(
        Window::new()
            .partition_by(Order::user_id)
            .order_by(Order::placed_on.asc()),
    );
    let query = Order::query().select((Order::id, previous.alias("previous_total")));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "orders"."id", LAG("orders"."total", $1) OVER (PARTITION BY "orders"."user_id" ORDER BY "orders"."placed_on" ASC) AS "previous_total" FROM "orders""#,
        &[int(1)],
    );
}

#[test]
fn ntile_splits_the_partition_into_buckets() {
    let quartile = ntile(4).over(Window::new().order_by(Department::budget.desc()));
    let query = Department::query().select((Department::name, quartile.alias("quartile")));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "departments"."name", NTILE($1) OVER (ORDER BY "departments"."budget" DESC) AS "quartile" FROM "departments""#,
        &[int(4)],
    );
}

#[test]
fn an_empty_window_covers_the_whole_result_set() {
    let share = Order::total.sum().over(Window::new()).alias("grand_total");
    let query = Order::query().select((Order::id, share));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "orders"."id", SUM("orders"."total") OVER () AS "grand_total" FROM "orders""#,
    );
}

#[test]
fn window_ordering_is_emulated_for_nulls_the_same_way_the_outer_query_is() {
    let latest = Post::views.max().over(
        Window::new()
            .partition_by(Post::user_id)
            .order_by(Post::published_at.desc().nulls_last()),
    );
    let query = Post::query().select(latest);
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT MAX("posts"."views") OVER (PARTITION BY "posts"."user_id" ORDER BY "posts"."published_at" DESC NULLS LAST) FROM "posts""#,
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT MAX("posts"."views") OVER (PARTITION BY "posts"."user_id" ORDER BY CASE WHEN "posts"."published_at" IS NULL THEN 1 ELSE 0 END, "posts"."published_at" DESC) FROM "posts""#,
    );
}

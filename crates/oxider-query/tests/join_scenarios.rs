//! JOIN scenarios, including self-joins, multi-table joins and derived tables.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite};

#[test]
fn an_inner_join_puts_the_joined_entity_in_scope_for_its_own_condition() {
    let query = User::query()
        .inner_join(Post::table(), Post::user_id.eq(User::id))
        .select((User::name, Post::title));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."name", "posts"."title" FROM "users" INNER JOIN "posts" ON "posts"."user_id" = "users"."id""#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT `users`.`name`, `posts`.`title` FROM `users` \
         INNER JOIN `posts` ON `posts`.`user_id` = `users`.`id`",
    );
}

#[test]
fn joins_chain_and_each_one_widens_the_scope_for_the_next() {
    let query = User::query()
        .inner_join(Order::table(), Order::user_id.eq(User::id))
        .inner_join(OrderItem::table(), OrderItem::order_id.eq(Order::id))
        .filter(User::active.eq(true).and(OrderItem::quantity.gt(1)))
        .select((User::name, OrderItem::product, OrderItem::quantity));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "users"."name", "order_items"."product", "order_items"."quantity" FROM "users" INNER JOIN "orders" ON "orders"."user_id" = "users"."id" INNER JOIN "order_items" ON "order_items"."order_id" = "orders"."id" WHERE "users"."active" = $1 AND "order_items"."quantity" > $2"#,
        &[flag(true), int(1)],
    );
}

#[test]
fn a_left_join_keeps_rows_with_no_match() {
    let query = User::query()
        .left_join(Post::table(), Post::user_id.eq(User::id))
        .filter(Post::id.is_null())
        .select(User::name);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name" FROM "users" LEFT JOIN "posts" ON "posts"."user_id" = "users"."id" WHERE "posts"."id" IS NULL"#,
    );
}

#[test]
fn a_self_join_distinguishes_the_two_copies_by_alias() {
    let query = User::query()
        .inner_join(
            User::table().alias("manager"),
            User::id.at("manager").eq(User::manager_id),
        )
        .select((User::name, User::name.at("manager")));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name", "manager"."name" FROM "users" INNER JOIN "users" AS "manager" ON "manager"."id" = "users"."manager_id""#,
    );
}

#[test]
fn a_cross_join_has_no_condition() {
    let query = User::query()
        .cross_join(Department::table())
        .select((User::name, Department::name));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name", "departments"."name" FROM "users" CROSS JOIN "departments""#,
    );
}

#[test]
fn a_right_join_renders_where_the_engine_has_one() {
    let query = User::query().right_join(Post::table(), Post::user_id.eq(User::id));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" RIGHT JOIN "posts" ON "posts"."user_id" = "users"."id""#,
    );
    assert_rejected(query, &Sqlite, "RIGHT JOIN");
}

#[test]
fn a_full_join_is_refused_by_mysql_and_sqlite() {
    let query = User::query().full_join(Post::table(), Post::user_id.eq(User::id));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" FULL JOIN "posts" ON "posts"."user_id" = "users"."id""#,
    );
    assert_rejected(query.clone(), &MySql, "FULL JOIN");
    assert_rejected(query, &Sqlite, "FULL JOIN");
}

#[test]
fn a_condition_may_combine_the_join_key_with_a_filter() {
    let query = User::query().inner_join(
        Post::table(),
        Post::user_id.eq(User::id).and(Post::views.gt(100)),
    );
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" INNER JOIN "posts" ON "posts"."user_id" = "users"."id" AND "posts"."views" > $1"#,
        &[int(100)],
    );
}

#[test]
fn a_derived_table_is_joined_by_alias_and_its_columns_read_through_col() {
    let totals = Order::query()
        .select((Order::user_id, Order::total.sum().alias("spent")))
        .group_by(Order::user_id);

    let query = User::query()
        .join_query(
            totals,
            "totals",
            col::<i64>("totals", "user_id").eq(User::id),
        )
        .select((User::name, col::<f64>("totals", "spent")));

    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name", "totals"."spent" FROM "users" INNER JOIN (SELECT "orders"."user_id", SUM("orders"."total") AS "spent" FROM "orders" GROUP BY "orders"."user_id") AS "totals" ON "totals"."user_id" = "users"."id""#,
    );
}

#[test]
fn an_extra_from_entry_behaves_as_a_comma_join() {
    let query = User::query()
        .and_from(Department::table())
        .filter(User::department_id.eq(Department::id))
        .select((User::name, Department::name));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name", "departments"."name" FROM "users", "departments" WHERE "users"."department_id" = "departments"."id""#,
    );
}

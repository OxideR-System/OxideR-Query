//! Set operations and common table expressions, plus the dialect capabilities
//! that govern them.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite};

#[test]
fn a_union_combines_two_queries_and_wraps_the_branch_where_the_engine_expects_it() {
    let query = User::query()
        .select(User::name)
        .union(Department::query().select(Department::name));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."name" FROM "users" UNION (SELECT "departments"."name" FROM "departments")"#,
    );
    // SQLite rejects a parenthesised compound branch, so it gets the bare form.
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT "users"."name" FROM "users" UNION SELECT "departments"."name" FROM "departments""#,
    );
}

#[test]
fn union_all_keeps_duplicates() {
    let query = User::query()
        .select(User::name)
        .union_all(Department::query().select(Department::name));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name" FROM "users" UNION ALL (SELECT "departments"."name" FROM "departments")"#,
    );
}

#[test]
fn the_outer_ordering_and_limit_apply_to_the_whole_set_operation() {
    let query = User::query()
        .select(User::name)
        .union(Department::query().select(Department::name))
        .order_by(User::name.asc())
        .limit(10);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "users"."name" FROM "users" UNION (SELECT "departments"."name" FROM "departments") ORDER BY "users"."name" ASC LIMIT 10"#,
    );
}

#[test]
fn intersect_and_except_are_refused_on_mysql() {
    let intersect = User::query()
        .select(User::name)
        .intersect(Department::query().select(Department::name));
    assert_sql_only(
        intersect.clone(),
        &Postgres,
        r#"SELECT "users"."name" FROM "users" INTERSECT (SELECT "departments"."name" FROM "departments")"#,
    );
    assert_rejected(intersect, &MySql, "INTERSECT");

    let except = User::query()
        .select(User::name)
        .except(Department::query().select(Department::name));
    assert_sql_only(
        except.clone(),
        &Postgres,
        r#"SELECT "users"."name" FROM "users" EXCEPT (SELECT "departments"."name" FROM "departments")"#,
    );
    assert_rejected(except, &MySql, "EXCEPT");
}

#[test]
fn the_all_form_of_intersect_is_refused_where_only_the_plain_form_exists() {
    let query = User::query()
        .select(User::name)
        .intersect_all(Department::query().select(Department::name));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."name" FROM "users" INTERSECT ALL (SELECT "departments"."name" FROM "departments")"#,
    );
    assert_rejected(query, &Sqlite, "ALL form");
}

#[test]
fn a_cte_is_declared_ahead_of_the_query_that_uses_it() {
    let busy = Post::query()
        .select((Post::user_id, count_all().alias("post_count")))
        .group_by(Post::user_id)
        .having(count_all().gt(10));

    // The CTE joins the FROM list as a named source; its columns are reached
    // with `col`, because no entity describes them.
    let query = User::query()
        .with("busy", busy)
        .join_query(
            oxider_query::select_from_name("busy").select(col::<i64>("busy", "user_id")),
            "b",
            User::id.eq(col::<i64>("b", "user_id")),
        )
        .select(User::name);

    assert_sql(
        query,
        &Postgres,
        r#"WITH "busy" AS (SELECT "posts"."user_id", COUNT(*) AS "post_count" FROM "posts" GROUP BY "posts"."user_id" HAVING COUNT(*) > $1) SELECT "users"."name" FROM "users" INNER JOIN (SELECT "busy"."user_id" FROM "busy") AS "b" ON "users"."id" = "b"."user_id""#,
        &[int(10)],
    );
}

#[test]
fn a_query_reads_a_cte_as_if_it_were_a_table() {
    let recent = Post::query()
        .filter(Post::views.gt(100))
        .select((Post::user_id, Post::title));

    let query = oxider_query::select_from_name("recent")
        .with("recent", recent)
        .select((
            col::<i64>("recent", "user_id"),
            col::<String>("recent", "title"),
        ))
        .order_by(col::<String>("recent", "title").asc());

    assert_sql(
        query,
        &Postgres,
        r#"WITH "recent" AS (SELECT "posts"."user_id", "posts"."title" FROM "posts" WHERE "posts"."views" > $1) SELECT "recent"."user_id", "recent"."title" FROM "recent" ORDER BY "recent"."title" ASC"#,
        &[int(100)],
    );
}

#[test]
fn a_cte_may_name_its_own_columns() {
    let totals = Order::query()
        .select((Order::user_id, Order::total.sum()))
        .group_by(Order::user_id);

    let query = oxider_query::select_from_name("totals")
        .with_columns("totals", ["user_id", "spent"], totals)
        .select(col::<f64>("totals", "spent"));

    assert_sql_only(
        query,
        &Postgres,
        r#"WITH "totals" ("user_id", "spent") AS (SELECT "orders"."user_id", SUM("orders"."total") FROM "orders" GROUP BY "orders"."user_id") SELECT "totals"."spent" FROM "totals""#,
    );
}

#[test]
fn a_recursive_cte_walks_a_management_chain() {
    // The anchor: the people with no manager. The recursive branch joins the
    // CTE back to the table, which is what `select_from_name` and `col` are for.
    let anchor = User::query()
        .filter(User::manager_id.is_null())
        .select((User::id, User::name));

    let step = User::query()
        .join_name_as("chain", "c", User::manager_id.eq(col::<i64>("c", "id")))
        .select((User::id, User::name));

    let query = oxider_query::select_from_name("chain")
        .with("chain", anchor.union_all(step))
        .recursive()
        .select(col::<String>("chain", "name"));

    assert_sql_only(
        query,
        &Postgres,
        r#"WITH RECURSIVE "chain" AS (SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."manager_id" IS NULL UNION ALL (SELECT "users"."id", "users"."name" FROM "users" INNER JOIN "chain" AS "c" ON "users"."manager_id" = "c"."id")) SELECT "chain"."name" FROM "chain""#,
    );
}

#[test]
fn a_branch_carrying_its_own_tail_is_refused_where_branches_are_not_wrapped() {
    // Postgres and MySQL parenthesise each branch, so a branch keeping its own
    // LIMIT is well defined. SQLite rejects a parenthesised branch, so the same
    // query would render as `... UNION SELECT ... LIMIT 5`, where the LIMIT
    // silently rebinds to the whole union instead of to the branch.
    let query = || {
        User::query()
            .select(User::name)
            .union(Department::query().select(Department::name).limit(5))
    };
    assert_sql_only(
        query(),
        &Postgres,
        r#"SELECT "users"."name" FROM "users" UNION (SELECT "departments"."name" FROM "departments" LIMIT 5)"#,
    );
    assert_rejected(query(), &Sqlite, "set-operation branch");
}

#[test]
fn a_branch_ordering_itself_is_refused_on_the_same_grounds() {
    let query = User::query().select(User::name).union(
        Department::query()
            .select(Department::name)
            .order_by(Department::name.asc()),
    );
    assert_rejected(query, &Sqlite, "set-operation branch");
}

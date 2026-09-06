//! The examples printed in `docs/`, pinned.
//!
//! Every SQL snippet the guide shows a reader is either copied from one of the
//! other scenario files or asserted here. Documentation that drifts from the
//! code is worse than no documentation, and a guide is exactly the kind of
//! thing that rots silently, so it gets the same treatment as any other
//! contract: a test that fails when it stops being true.
//!
//! Each test names the chapter it backs.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite};

/// Chapter 1: the first query, rendered for all three dialects.
#[test]
fn getting_started_shows_one_query_across_three_dialects() {
    let query = User::query()
        .filter(User::age.ge(18))
        .order_by(User::name.asc());
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."age" >= $1 ORDER BY "users"."name" ASC"#,
        &[int(18)],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT * FROM `users` WHERE `users`.`age` >= ? ORDER BY `users`.`name` ASC",
        &[int(18)],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT * FROM "users" WHERE "users"."age" >= ? ORDER BY "users"."name" ASC"#,
        &[int(18)],
    );
}

/// Chapter 1: the crate-level example, including the escaped LIKE.
#[test]
fn getting_started_shows_the_thirty_second_example() {
    let query = User::query()
        .select((User::id, User::name))
        .filter(User::name.contains("nguyen").and(User::age.ge(18)))
        .order_by(User::id.desc())
        .limit(20);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."name" LIKE $1 ESCAPE '!' AND "users"."age" >= $2 ORDER BY "users"."id" DESC LIMIT 20"#,
        &[text("%nguyen%"), int(18)],
    );
}

/// Chapter 2: a query begun from an aliased table.
#[test]
fn entities_chapter_shows_a_query_over_an_aliased_table() {
    assert_sql_only(
        User::query_as("u").select(User::name.at("u")),
        &Postgres,
        r#"SELECT "u"."name" FROM "users" AS "u""#,
    );
}

/// Chapter 3: `add_select` appends to an existing projection.
#[test]
fn select_chapter_shows_add_select_appending() {
    assert_sql_only(
        User::query().select(User::id).add_select(User::name),
        &Postgres,
        r#"SELECT "users"."id", "users"."name" FROM "users""#,
    );
}

/// Chapter 3: a dynamic search with both filters absent, then both present.
#[test]
fn select_chapter_shows_a_dynamic_search() {
    let none_at_all = User::query()
        .filter_opt(None::<&str>.map(|n| User::name.contains(n)))
        .filter_opt(None::<i32>.map(|a| User::age.ge(a)));
    assert_sql_only(none_at_all, &Postgres, r#"SELECT * FROM "users""#);

    let both = User::query()
        .filter_opt(Some("ada").map(|n| User::name.contains(n)))
        .filter_opt(Some(18).map(|a| User::age.ge(a)));
    assert_sql(
        both,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."name" LIKE $1 ESCAPE '!' AND "users"."age" >= $2"#,
        &[text("%ada%"), int(18)],
    );
}

/// Chapter 3: plain `limit`, with no offset.
#[test]
fn select_chapter_shows_a_bare_limit() {
    assert_sql_only(
        User::query().limit(25),
        &Postgres,
        r#"SELECT * FROM "users" LIMIT 25"#,
    );
}

/// Chapter 4: a regular-expression match, spelled differently everywhere.
#[test]
fn operators_chapter_shows_regex_matching_per_dialect() {
    let query = Post::query().filter(Post::title.matches("^draft"));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "posts" WHERE "posts"."title" ~ $1"#,
        &[text("^draft")],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT * FROM `posts` WHERE `posts`.`title` REGEXP BINARY ?",
        &[text("^draft")],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT * FROM "posts" WHERE "posts"."title" REGEXP ?"#,
        &[text("^draft")],
    );
}

/// Chapter 4: padding exists on two engines and is refused on the third.
#[test]
fn operators_chapter_shows_padding_refused_on_sqlite() {
    let query = User::query().select(User::name.pad_start(10, "0"));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT LPAD("users"."name", $1, $2) FROM "users""#,
        &[int(10), text("0")],
    );
    assert_rejected(query, &Sqlite, "LPad");
}

/// Chapter 4: `LEAST` keeps its ANSI name on MySQL.
#[test]
fn operators_chapter_shows_least_named_natively_on_mysql() {
    assert_sql(
        OrderItem::query().select(least(OrderItem::price).or(100.0).end()),
        &MySql,
        "SELECT LEAST(`order_items`.`price`, ?) FROM `order_items`",
        &[real(100.0)],
    );
}

/// Chapter 4: the three ways to say "every column".
#[test]
fn operators_chapter_shows_the_star_expressions() {
    assert_sql_only(
        User::query().select((star(), User::id)),
        &Postgres,
        r#"SELECT *, "users"."id" FROM "users""#,
    );
    assert_sql_only(
        User::query().select((star_of("users"), User::id)),
        &Postgres,
        r#"SELECT "users".*, "users"."id" FROM "users""#,
    );
    assert_sql_only(
        User::query().select((all_of(User::id), User::name)),
        &Postgres,
        r#"SELECT "users".*, "users"."name" FROM "users""#,
    );
}

/// Chapter 4: `random` is spelled differently on MySQL.
#[test]
fn operators_chapter_shows_random_per_dialect() {
    let query = User::query().select(User::id).order_by(random().asc());
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."id" FROM "users" ORDER BY RANDOM() ASC"#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT `users`.`id` FROM `users` ORDER BY RAND() ASC",
    );
}

/// Chapter 6: `distinct` applies to aggregates other than `COUNT` too.
#[test]
fn aggregate_chapter_shows_distinct_on_a_valued_aggregate() {
    assert_sql_only(
        Order::query().select(Order::total.sum().distinct()),
        &Postgres,
        r#"SELECT SUM(DISTINCT "orders"."total") FROM "orders""#,
    );
}

/// Chapter 6: `FILTER` is native on SQLite as well as PostgreSQL.
#[test]
fn aggregate_chapter_shows_a_native_filter_on_sqlite() {
    assert_sql(
        Order::query()
            .select(count_all().filter_where(Order::status.eq("paid")))
            .group_by(Order::user_id),
        &Sqlite,
        r#"SELECT COUNT(*) FILTER (WHERE "orders"."status" = ?) FROM "orders" GROUP BY "orders"."user_id""#,
        &[text("paid")],
    );
}

/// Chapter 9: MySQL wraps set-operation branches, the same as PostgreSQL.
#[test]
fn set_operation_chapter_shows_mysql_wrapping_branches() {
    assert_sql_only(
        User::query()
            .select(User::name)
            .union(Department::query().select(Department::name)),
        &MySql,
        "SELECT `users`.`name` FROM `users` UNION (SELECT `departments`.`name` FROM `departments`)",
    );
}

/// Chapter 10: an optional filter on the two write builders.
#[test]
fn dml_chapter_shows_optional_filters_on_writes() {
    let no_filter: Option<Predicate<Only<Post>>> = None;
    assert_sql(
        Post::update().set(Post::views, 0).filter_opt(no_filter),
        &Postgres,
        r#"UPDATE "posts" SET "views" = $1"#,
        &[int(0)],
    );

    let a_filter = Some(Post::views.eq(0));
    assert_sql(
        Post::delete().filter_opt(a_filter),
        &Postgres,
        r#"DELETE FROM "posts" WHERE "posts"."views" = $1"#,
        &[int(0)],
    );
}

/// Chapter 11: a dialect refusing a construct reports which one, and why.
#[test]
fn dialect_chapter_shows_what_a_refusal_says() {
    let err = Department::query()
        .select(Department::budget.std_dev())
        .to_sql(&Sqlite)
        .expect_err("SQLite has no STDDEV");
    assert_eq!(err.to_string(), "sqlite cannot express the operator StdDev");

    let err = User::query()
        .distinct_on(User::department_id)
        .to_sql(&MySql)
        .expect_err("MySQL has no DISTINCT ON");
    assert_eq!(err.to_string(), "mysql does not support DISTINCT ON");
}

//! Expression scenarios: pattern matching, arithmetic, conditionals, casts,
//! date arithmetic and the raw-SQL escape hatch.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{CastKind, MySql, Postgres, Sqlite};

#[test]
fn a_literal_search_escapes_the_users_wildcards() {
    // The classic bug this library exists to prevent: searching for the text
    // "50%" must not match "500 units", so the `%` is escaped and an explicit
    // ESCAPE clause pins the escape character.
    let query = Post::query().filter(Post::title.contains("50%"));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "posts" WHERE "posts"."title" LIKE $1 ESCAPE '!'"#,
        &[text("%50!%%")],
    );
    assert_sql(
        query,
        &MySql,
        "SELECT * FROM `posts` WHERE `posts`.`title` LIKE ? ESCAPE '!'",
        &[text("%50!%%")],
    );
}

#[test]
fn an_underscore_and_the_escape_character_itself_are_escaped_too() {
    let query = Post::query().filter(Post::title.starts_with("a_b!c"));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "posts" WHERE "posts"."title" LIKE $1 ESCAPE '!'"#,
        &[text("a!_b!!c%")],
    );
}

#[test]
fn a_raw_like_pattern_is_passed_through_untouched() {
    let query = Post::query().filter(Post::title.like("draft%"));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "posts" WHERE "posts"."title" LIKE $1"#,
        &[text("draft%")],
    );
}

#[test]
fn case_insensitive_matching_uses_ilike_on_postgres_and_lower_elsewhere() {
    let query = Post::query().filter(Post::title.contains_ignore_case("rust"));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "posts" WHERE "posts"."title" ILIKE $1 ESCAPE '!'"#,
        &[text("%rust%")],
    );
    assert_sql(
        query,
        &MySql,
        "SELECT * FROM `posts` WHERE LOWER(`posts`.`title`) LIKE LOWER(?) ESCAPE '!'",
        &[text("%rust%")],
    );
}

#[test]
fn string_functions_differ_by_dialect_but_mean_the_same_thing() {
    let query = User::query().select(User::name.substr_len(2, 5));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT SUBSTRING("users"."name" FROM $1 FOR $2) FROM "users""#,
        &[int(2), int(5)],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT SUBSTRING(`users`.`name`, ?, ?) FROM `users`",
        &[int(2), int(5)],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT SUBSTR("users"."name", ?, ?) FROM "users""#,
        &[int(2), int(5)],
    );
}

#[test]
fn position_reverses_its_arguments_on_the_dialects_that_need_it() {
    let query = User::query().select(User::name.index_of("@"));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT POSITION($1 IN "users"."name") FROM "users""#,
        &[text("@")],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT LOCATE(?, `users`.`name`) FROM `users`",
        &[text("@")],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT INSTR("users"."name", ?) FROM "users""#,
        &[text("@")],
    );
}

#[test]
fn concatenation_is_an_operator_everywhere_but_mysql() {
    let query = User::query().select(User::name.concat(User::email));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."name" || "users"."email" FROM "users""#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT CONCAT(`users`.`name`, `users`.`email`) FROM `users`",
    );
}

#[test]
fn arithmetic_is_parenthesised_by_precedence_not_by_the_template() {
    let no_parens = OrderItem::query().select(OrderItem::price.mul(2.0).add(1.0));
    assert_sql(
        no_parens,
        &Postgres,
        r#"SELECT "order_items"."price" * $1 + $2 FROM "order_items""#,
        &[real(2.0), real(1.0)],
    );

    let parens = OrderItem::query().select(OrderItem::price.add(1.0).mul(2.0));
    assert_sql(
        parens,
        &Postgres,
        r#"SELECT ("order_items"."price" + $1) * $2 FROM "order_items""#,
        &[real(1.0), real(2.0)],
    );
}

#[test]
fn a_line_total_multiplies_two_columns() {
    let query = OrderItem::query()
        .select(OrderItem::price.mul(OrderItem::quantity.cast::<f64>(CastKind::Float)))
        .filter(OrderItem::order_id.eq(7));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "order_items"."price" * CAST("order_items"."quantity" AS DOUBLE PRECISION) FROM "order_items" WHERE "order_items"."order_id" = $1"#,
        &[int(7)],
    );
}

#[test]
fn modulo_is_an_operator_on_postgres_and_sqlite_and_a_function_in_ansi() {
    let query = Post::query().filter(Post::views.rem(2).eq(0));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "posts" WHERE "posts"."views" % $1 = $2"#,
        &[int(2), int(0)],
    );
    assert_sql(
        query.clone(),
        &Sqlite,
        r#"SELECT * FROM "posts" WHERE "posts"."views" % ? = ?"#,
        &[int(2), int(0)],
    );
    assert_sql(
        query,
        &MySql,
        "SELECT * FROM `posts` WHERE MOD(`posts`.`views`, ?) = ?",
        &[int(2), int(0)],
    );
}

#[test]
fn a_cast_names_the_target_type_per_dialect() {
    let query = User::query().select(User::age.cast::<String>(CastKind::Text));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT CAST("users"."age" AS TEXT) FROM "users""#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT CAST(`users`.`age` AS CHAR) FROM `users`",
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT CAST("users"."age" AS TEXT) FROM "users""#,
    );
}

#[test]
fn a_case_expression_buckets_rows() {
    let bucket = case_when(User::age.lt(18), "minor")
        .when(User::age.lt(65), "adult")
        .otherwise("senior");
    let query = User::query().select((User::name, bucket.alias("bracket")));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "users"."name", CASE WHEN "users"."age" < $1 THEN $2 WHEN "users"."age" < $3 THEN $4 ELSE $5 END AS "bracket" FROM "users""#,
        &[
            int(18),
            text("minor"),
            int(65),
            text("adult"),
            text("senior"),
        ],
    );
}

#[test]
fn a_case_with_no_else_leaves_unmatched_rows_null() {
    let flagged = case_when(User::active.eq(true), 1i64).end();
    let query = User::query().select(flagged);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT CASE WHEN "users"."active" = $1 THEN $2 END FROM "users""#,
        &[flag(true), int(1)],
    );
}

#[test]
fn coalesce_supplies_a_default_for_a_nullable_column() {
    let query = User::query().select(coalesce(User::email).or("none@example.com").end());
    assert_sql(
        query,
        &Postgres,
        r#"SELECT COALESCE("users"."email", $1) FROM "users""#,
        &[text("none@example.com")],
    );
}

#[test]
fn nullif_turns_a_sentinel_into_null() {
    let query = User::query().select(nullif(User::name, ""));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT NULLIF("users"."name", $1) FROM "users""#,
        &[text("")],
    );
}

#[test]
fn least_and_greatest_are_named_min_and_max_on_sqlite() {
    let query = OrderItem::query().select(least(OrderItem::price).or(100.0).end());
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT LEAST("order_items"."price", $1) FROM "order_items""#,
        &[real(100.0)],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT MIN("order_items"."price", ?) FROM "order_items""#,
        &[real(100.0)],
    );
}

#[test]
fn extracting_a_date_part_reads_the_same_on_every_engine() {
    let query = User::query().select(User::created_at.year());
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT EXTRACT(YEAR FROM "users"."created_at") FROM "users""#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT YEAR(`users`.`created_at`) FROM `users`",
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT CAST(STRFTIME('%Y', "users"."created_at") AS INTEGER) FROM "users""#,
    );
}

#[test]
fn the_day_of_week_is_normalised_so_sunday_is_one_everywhere() {
    let query = User::query().filter(User::created_at.day_of_week().eq(1));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" WHERE (EXTRACT(DOW FROM "users"."created_at") + 1) = $1"#,
        &[int(1)],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT * FROM `users` WHERE DAYOFWEEK(`users`.`created_at`) = ?",
        &[int(1)],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT * FROM "users" WHERE (CAST(STRFTIME('%w', "users"."created_at") AS INTEGER) + 1) = ?"#,
        &[int(1)],
    );
}

#[test]
fn date_arithmetic_uses_each_engines_own_interval_syntax() {
    let query = User::query().select(User::created_at.add_days(7));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT ("users"."created_at" + ($1 * INTERVAL '1 day')) FROM "users""#,
        &[int(7)],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT DATE_ADD(`users`.`created_at`, INTERVAL ? DAY) FROM `users`",
        &[int(7)],
    );
    assert_sql(
        query,
        &Sqlite,
        r#"SELECT DATETIME("users"."created_at", ? || ' days') FROM "users""#,
        &[int(7)],
    );
}

#[test]
fn truncating_to_a_month_is_emulated_where_there_is_no_date_trunc() {
    let query = User::query().select(User::created_at.truncate_to_month());
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT DATE_TRUNC('month', "users"."created_at") FROM "users""#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT DATE_SUB(DATE(`users`.`created_at`), INTERVAL DAYOFMONTH(`users`.`created_at`) - 1 DAY) FROM `users`",
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT DATE("users"."created_at", 'start of month') FROM "users""#,
    );
}

#[test]
fn a_difference_in_days_binds_the_comparison_instant_once_per_appearance() {
    let query = User::query().select(User::created_at.diff_days(a_timestamp()));
    // The PostgreSQL template mentions each operand once.
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT CAST(EXTRACT(EPOCH FROM ($1 - "users"."created_at")) / 86400 AS INTEGER) FROM "users""#,
        &[timestamp("2024-03-15 09:30:00")],
    );
    assert_sql(
        query,
        &MySql,
        "SELECT TIMESTAMPDIFF(DAY, `users`.`created_at`, ?) FROM `users`",
        &[timestamp("2024-03-15 09:30:00")],
    );
}

#[test]
fn the_current_timestamp_needs_no_parameter() {
    let query = User::query().filter(User::created_at.lt(now()));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."created_at" < CURRENT_TIMESTAMP"#,
    );
}

#[test]
fn a_raw_fragment_writes_sql_verbatim_and_still_binds_its_values() {
    let json_path = oxider_query::typed::Raw::new()
        .expr(Post::body)
        .sql(" ->> ")
        .bind("author")
        .build::<String>();
    let query = Post::query().select(json_path.alias("author"));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT "posts"."body" ->> $1 AS "author" FROM "posts""#,
        &[text("author")],
    );
}

#[test]
fn a_boolean_column_combines_without_a_comparison() {
    let query = User::query().filter(User::active.not().and(User::age.gt(65)));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE NOT "users"."active" AND "users"."age" > $1"#,
        &[int(65)],
    );
}

#[test]
fn xor_is_native_on_mysql_and_expressed_as_inequality_elsewhere() {
    let query = User::query().filter(User::active.xor(User::email.is_null()));
    // `<>` binds tighter than `IS NULL`, so the right operand needs wrapping;
    // MySQL's XOR keyword binds looser than `IS NULL`, so it does not.
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."active" <> ("users"."email" IS NULL)"#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT * FROM `users` WHERE `users`.`active` XOR `users`.`email` IS NULL",
    );
}

#[test]
fn a_null_test_used_as_an_operand_is_parenthesised() {
    let query = User::query().select(User::email.is_null().eq(User::active));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT ("users"."email" IS NULL) = "users"."active" FROM "users""#,
    );
}

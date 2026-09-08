//! SELECT scenarios: projection, filtering, ordering, paging and locking.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite};

#[test]
fn a_query_with_no_projection_selects_every_column() {
    let query = User::query();
    assert_sql_only(query.clone(), &Postgres, r#"SELECT * FROM "users""#);
    assert_sql_only(query.clone(), &MySql, "SELECT * FROM `users`");
    assert_sql_only(query, &Sqlite, r#"SELECT * FROM "users""#);
}

#[test]
fn a_projection_selects_exactly_the_listed_columns_in_order() {
    let query = User::query().select((User::id, User::name, User::age));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."id", "users"."name", "users"."age" FROM "users""#,
    );
    assert_sql_only(
        query,
        &MySql,
        "SELECT `users`.`id`, `users`.`name`, `users`.`age` FROM `users`",
    );
}

#[test]
fn an_alias_names_a_computed_column() {
    let query = User::query().select(User::name.upper().alias("shouted"));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT UPPER("users"."name") AS "shouted" FROM "users""#,
    );
}

#[test]
fn repeated_filters_are_combined_with_and() {
    let query = User::query()
        .filter(User::age.ge(18))
        .filter(User::active.eq(true));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."age" >= $1 AND "users"."active" = $2"#,
        &[int(18), flag(true)],
    );
}

#[test]
fn an_or_inside_an_and_is_parenthesised_but_the_reverse_is_not() {
    let grouped = User::query().filter(
        User::age
            .lt(18)
            .or(User::age.gt(65))
            .and(User::active.eq(true)),
    );
    assert_sql(
        grouped,
        &Postgres,
        r#"SELECT * FROM "users" WHERE ("users"."age" < $1 OR "users"."age" > $2) AND "users"."active" = $3"#,
        &[int(18), int(65), flag(true)],
    );

    let flat = User::query().filter(
        User::age
            .lt(18)
            .and(User::active.eq(true))
            .or(User::age.gt(65)),
    );
    assert_sql(
        flat,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."age" < $1 AND "users"."active" = $2 OR "users"."age" > $3"#,
        &[int(18), flag(true), int(65)],
    );
}

#[test]
fn placeholder_numbering_follows_the_order_values_appear() {
    let query = User::query()
        .select(User::id)
        .filter(User::name.eq("ada").and(User::age.between(30, 40)));
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT "users"."id" FROM "users" WHERE "users"."name" = $1 AND "users"."age" BETWEEN $2 AND $3"#,
        &[text("ada"), int(30), int(40)],
    );
    // Positional dialects bind the same values in the same order.
    assert_sql(
        query,
        &MySql,
        "SELECT `users`.`id` FROM `users` WHERE `users`.`name` = ? AND `users`.`age` BETWEEN ? AND ?",
        &[text("ada"), int(30), int(40)],
    );
}

#[test]
fn an_empty_in_list_becomes_a_predicate_that_is_never_true() {
    let ids: Vec<i64> = Vec::new();
    let query = User::query().filter(User::id.in_values(ids));
    assert_sql_only(query, &Postgres, r#"SELECT * FROM "users" WHERE (1 = 0)"#);
}

#[test]
fn an_empty_not_in_list_becomes_a_predicate_that_is_always_true() {
    let ids: Vec<i64> = Vec::new();
    let query = User::query().filter(User::id.not_in_values(ids));
    assert_sql_only(query, &Postgres, r#"SELECT * FROM "users" WHERE (1 = 1)"#);
}

#[test]
fn the_empty_membership_constant_composes_like_any_other_predicate() {
    // The constant carries its own parentheses, because the renderer treats a
    // keyword as an atom that never needs wrapping. Without them, composing it
    // further produced `1 = 0 = $1`.
    let ids: Vec<i64> = Vec::new();
    assert_sql(
        User::query().filter(User::id.in_values(ids).eq(true)),
        &Postgres,
        r#"SELECT * FROM "users" WHERE (1 = 0) = $1"#,
        &[flag(true)],
    );
}

#[test]
fn an_in_list_binds_one_parameter_per_value() {
    let query = User::query().filter(User::id.in_values([1i64, 2, 3]));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."id" IN ($1, $2, $3)"#,
        &[int(1), int(2), int(3)],
    );
}

#[test]
fn null_tests_render_without_binding_anything() {
    let query = User::query().filter(User::email.is_null().or(User::email.is_not_null()));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."email" IS NULL OR "users"."email" IS NOT NULL"#,
    );
}

#[test]
fn null_safe_equality_uses_each_dialects_own_spelling() {
    let query = User::query().filter(User::manager_id.is_not_distinct_from(User::department_id));
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."manager_id" IS NOT DISTINCT FROM "users"."department_id""#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT * FROM `users` WHERE `users`.`manager_id` <=> `users`.`department_id`",
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT * FROM "users" WHERE "users"."manager_id" IS "users"."department_id""#,
    );
}

#[test]
fn ordering_appends_terms_in_the_order_they_are_added() {
    let query = User::query()
        .order_by(User::age.desc())
        .order_by(User::name.asc());
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT * FROM "users" ORDER BY "users"."age" DESC, "users"."name" ASC"#,
    );
}

#[test]
fn null_placement_is_native_on_postgres_and_emulated_elsewhere() {
    let query = User::query().order_by(User::email.desc().nulls_last());
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" ORDER BY "users"."email" DESC NULLS LAST"#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT * FROM `users` ORDER BY CASE WHEN `users`.`email` IS NULL THEN 1 ELSE 0 END, \
         `users`.`email` DESC",
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT * FROM "users" ORDER BY CASE WHEN "users"."email" IS NULL THEN 1 ELSE 0 END, "users"."email" DESC"#,
    );
}

#[test]
fn nulls_first_emulation_sorts_the_null_group_ahead() {
    let query = User::query().order_by(User::email.asc().nulls_first());
    assert_sql_only(
        query,
        &MySql,
        "SELECT * FROM `users` ORDER BY CASE WHEN `users`.`email` IS NULL THEN 0 ELSE 1 END, \
         `users`.`email` ASC",
    );
}

#[test]
fn paging_renders_limit_and_offset() {
    let query = User::query().page(2, 25);
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" LIMIT 25 OFFSET 50"#,
    );
    assert_sql_only(query, &MySql, "SELECT * FROM `users` LIMIT 25 OFFSET 50");
}

#[test]
fn an_offset_with_no_limit_gets_the_placeholder_limit_engines_require() {
    let query = User::query().offset(10);
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" OFFSET 10"#,
    );
    assert_sql_only(
        query.clone(),
        &MySql,
        "SELECT * FROM `users` LIMIT 18446744073709551615 OFFSET 10",
    );
    assert_sql_only(
        query,
        &Sqlite,
        r#"SELECT * FROM "users" LIMIT -1 OFFSET 10"#,
    );
}

#[test]
fn distinct_applies_to_the_whole_projection() {
    let query = User::query().distinct().select(User::department_id);
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT DISTINCT "users"."department_id" FROM "users""#,
    );
}

#[test]
fn distinct_on_is_postgres_only_and_refused_elsewhere() {
    let query = User::query()
        .distinct_on(User::department_id)
        .select((User::department_id, User::name))
        .order_by(User::department_id.asc())
        .order_by(User::name.asc());
    assert_sql_only(
        query.clone(),
        &Postgres,
        r#"SELECT DISTINCT ON ("users"."department_id") "users"."department_id", "users"."name" FROM "users" ORDER BY "users"."department_id" ASC, "users"."name" ASC"#,
    );
    assert_rejected(query, &MySql, "DISTINCT ON");
}

#[test]
fn row_locking_renders_where_it_exists_and_is_refused_on_sqlite() {
    let query = User::query()
        .filter(User::id.eq(1))
        .for_update()
        .skip_locked();
    assert_sql(
        query.clone(),
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."id" = $1 FOR UPDATE SKIP LOCKED"#,
        &[int(1)],
    );
    assert_sql(
        query.clone(),
        &MySql,
        "SELECT * FROM `users` WHERE `users`.`id` = ? FOR UPDATE SKIP LOCKED",
        &[int(1)],
    );
    assert_rejected(query, &Sqlite, "row locking");
}

#[test]
fn a_schema_qualified_entity_carries_its_schema_into_from_and_column_refs() {
    let query = AuditLog::query().select((AuditLog::actor_name, AuditLog::action));
    assert_sql_only(
        query,
        &Postgres,
        r#"SELECT "audit_log"."actor", "audit_log"."action" FROM "ops"."audit_log""#,
    );
}

#[test]
fn a_from_less_select_renders_a_bare_projection() {
    let query = oxider_query::select_only(val(1i64).alias("one"));
    assert_sql(query, &Postgres, r#"SELECT $1 AS "one""#, &[int(1)]);
}

#[test]
fn an_optional_filter_is_skipped_when_absent_and_applied_when_present() {
    let none: Option<oxider_query::Predicate<oxider_query::Only<User>>> = None;
    let query = User::query().filter_opt(none);
    assert_sql_only(query, &Postgres, r#"SELECT * FROM "users""#);

    let some = Some(User::name.eq("ada"));
    let query = User::query().filter_opt(some);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."name" = $1"#,
        &[text("ada")],
    );
}

#[test]
fn a_temporal_column_compares_against_a_chrono_value() {
    let query = User::query().filter(User::created_at.ge(a_timestamp()));
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."created_at" >= $1"#,
        &[timestamp("2024-03-15 09:30:00")],
    );
}

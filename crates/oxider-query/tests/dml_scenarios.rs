//! INSERT, UPDATE and DELETE scenarios, including multi-row inserts, upserts,
//! `RETURNING`, and the multi-table forms.

mod common;

use common::*;
use oxider_query::builder::excluded;
use oxider_query::prelude::*;
use oxider_query::{MySql, Postgres, Sqlite};

#[test]
fn a_single_row_insert_lists_the_columns_it_sets() {
    let statement = User::insert()
        .set(User::name, "ada")
        .set(User::age, 36)
        .set(User::active, true);
    assert_sql(
        statement.clone(),
        &Postgres,
        r#"INSERT INTO "users" ("name", "age", "active") VALUES ($1, $2, $3)"#,
        &[text("ada"), int(36), flag(true)],
    );
    assert_sql(
        statement,
        &MySql,
        "INSERT INTO `users` (`name`, `age`, `active`) VALUES (?, ?, ?)",
        &[text("ada"), int(36), flag(true)],
    );
}

#[test]
fn a_multi_row_insert_repeats_the_values_list() {
    let statement = User::insert()
        .columns((User::name, User::age))
        .values(("ada", 36))
        .values(("grace", 45));
    assert_sql(
        statement,
        &Postgres,
        r#"INSERT INTO "users" ("name", "age") VALUES ($1, $2), ($3, $4)"#,
        &[text("ada"), int(36), text("grace"), int(45)],
    );
}

#[test]
fn an_insert_can_take_its_rows_from_a_query() {
    let statement = Post::insert()
        .columns((Post::user_id, Post::title))
        .from_query(
            User::query()
                .filter(User::active.eq(true))
                .select((User::id, User::name)),
        );
    assert_sql(
        statement,
        &Postgres,
        r#"INSERT INTO "posts" ("user_id", "title") SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."active" = $1"#,
        &[flag(true)],
    );
}

#[test]
fn an_upsert_updates_the_conflicting_row_with_the_value_that_failed_to_insert() {
    let statement = User::insert()
        .set(User::email, "ada@example.com")
        .set(User::name, "ada")
        .on_conflict(["email"])
        .do_update()
        .set(User::name, excluded(User::name))
        .end();

    assert_sql(
        statement.clone(),
        &Postgres,
        r#"INSERT INTO "users" ("email", "name") VALUES ($1, $2) ON CONFLICT ("email") DO UPDATE SET "name" = excluded."name""#,
        &[text("ada@example.com"), text("ada")],
    );
    assert_sql(
        statement.clone(),
        &Sqlite,
        r#"INSERT INTO "users" ("email", "name") VALUES (?, ?) ON CONFLICT ("email") DO UPDATE SET "name" = excluded."name""#,
        &[text("ada@example.com"), text("ada")],
    );
    // MySQL spells the same intent with its own clause, and refers to the
    // rejected row with `VALUES(...)` rather than `excluded`.
    assert_sql(
        statement,
        &MySql,
        "INSERT INTO `users` (`email`, `name`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `name` = VALUES(`name`)",
        &[text("ada@example.com"), text("ada")],
    );
}

#[test]
fn do_nothing_is_refused_on_mysql_which_has_no_equivalent() {
    let statement = User::insert()
        .set(User::email, "ada@example.com")
        .on_conflict(["email"])
        .do_nothing();
    assert_sql(
        statement.clone(),
        &Postgres,
        r#"INSERT INTO "users" ("email") VALUES ($1) ON CONFLICT ("email") DO NOTHING"#,
        &[text("ada@example.com")],
    );
    assert_rejected(statement, &MySql, "DO NOTHING");
}

#[test]
fn returning_hands_back_generated_columns_where_the_engine_supports_it() {
    let statement = User::insert()
        .set(User::name, "ada")
        .returning((User::id, User::created_at));
    assert_sql(
        statement.clone(),
        &Postgres,
        r#"INSERT INTO "users" ("name") VALUES ($1) RETURNING "users"."id", "users"."created_at""#,
        &[text("ada")],
    );
    assert_rejected(statement, &MySql, "RETURNING");
}

#[test]
fn an_update_assigns_from_an_expression_over_the_row_itself() {
    let statement = Post::update()
        .set(Post::views, Post::views.add(1))
        .filter(Post::id.eq(42));
    assert_sql(
        statement,
        &Postgres,
        r#"UPDATE "posts" SET "views" = "posts"."views" + $1 WHERE "posts"."id" = $2"#,
        &[int(1), int(42)],
    );
}

#[test]
fn an_update_sets_several_columns_in_the_order_given() {
    let statement = User::update()
        .set(User::name, "grace")
        .set(User::active, false)
        .set_null(User::email)
        .filter(User::id.eq(1));
    assert_sql(
        statement,
        &Postgres,
        r#"UPDATE "users" SET "name" = $1, "active" = $2, "email" = NULL WHERE "users"."id" = $3"#,
        &[text("grace"), flag(false), int(1)],
    );
}

#[test]
fn an_update_from_another_table_reads_that_tables_columns() {
    let statement = User::update()
        .from(Department::table())
        .set(User::name, Department::name)
        .filter(User::department_id.eq(Department::id));
    assert_sql_only(
        statement,
        &Postgres,
        r#"UPDATE "users" SET "name" = "departments"."name" FROM "departments" WHERE "users"."department_id" = "departments"."id""#,
    );
}

#[test]
fn an_update_from_another_table_is_refused_where_the_engine_has_no_such_form() {
    // MySQL spells this as a multi-table UPDATE and SQLite has no form at all,
    // so the Postgres shape would be a syntax error at the database.
    let statement = || {
        User::update()
            .from(Department::table())
            .set(User::name, Department::name)
            .filter(User::department_id.eq(Department::id))
    };
    assert_rejected(statement(), &MySql, "UPDATE ... FROM");
    assert_sql_only(
        statement(),
        &Sqlite,
        r#"UPDATE "users" SET "name" = "departments"."name" FROM "departments" WHERE "users"."department_id" = "departments"."id""#,
    );
}

#[test]
fn an_update_that_assigns_nothing_is_refused_rather_than_rendered() {
    // `UPDATE t SET WHERE ...` is a syntax error, and an UPDATE assigning
    // nothing cannot be what the caller meant either way.
    let statement = User::update().filter(User::id.eq(1));
    assert_rejected(statement, &Postgres, "at least one assignment");
}

#[test]
fn a_delete_with_no_condition_removes_every_row() {
    let statement = Post::delete();
    assert_sql_only(statement, &Postgres, r#"DELETE FROM "posts""#);
}

#[test]
fn a_delete_using_another_table_qualifies_the_rows_to_remove() {
    let statement = Post::delete()
        .using(User::table())
        .filter(Post::user_id.eq(User::id).and(User::active.eq(false)));
    assert_sql(
        statement,
        &Postgres,
        r#"DELETE FROM "posts" USING "users" WHERE "posts"."user_id" = "users"."id" AND "users"."active" = $1"#,
        &[flag(false)],
    );
}

#[test]
fn a_delete_using_another_table_is_refused_where_the_engine_has_no_such_form() {
    // MySQL's `USING` lists every table including the target, so the Postgres
    // shape is ERROR 1109 there; SQLite has no multi-table delete at all.
    let statement = || {
        Post::delete()
            .using(User::table())
            .filter(Post::user_id.eq(User::id))
    };
    assert_rejected(statement(), &MySql, "DELETE ... USING");
    assert_rejected(statement(), &Sqlite, "DELETE ... USING");
}

#[test]
fn a_delete_can_return_the_rows_it_removed() {
    let statement = Post::delete().filter(Post::views.eq(0)).returning(Post::id);
    assert_sql(
        statement.clone(),
        &Postgres,
        r#"DELETE FROM "posts" WHERE "posts"."views" = $1 RETURNING "posts"."id""#,
        &[int(0)],
    );
    assert_sql(
        statement,
        &Sqlite,
        r#"DELETE FROM "posts" WHERE "posts"."views" = ? RETURNING "posts"."id""#,
        &[int(0)],
    );
}

#[test]
fn a_delete_driven_by_a_subquery_needs_no_second_table_in_scope() {
    let inactive = User::query()
        .filter(User::active.eq(false))
        .scalar(User::id);
    let statement = Post::delete().filter(Post::user_id.in_subquery(inactive));
    assert_sql(
        statement,
        &Postgres,
        r#"DELETE FROM "posts" WHERE "posts"."user_id" IN (SELECT "users"."id" FROM "users" WHERE "users"."active" = $1)"#,
        &[flag(false)],
    );
}

#[test]
fn an_insert_into_a_schema_qualified_table_keeps_the_qualifier() {
    let statement = AuditLog::insert()
        .set(AuditLog::actor_name, "ada")
        .set(AuditLog::action, "login");
    assert_sql(
        statement,
        &Postgres,
        r#"INSERT INTO "ops"."audit_log" ("actor", "action") VALUES ($1, $2)"#,
        &[text("ada"), text("login")],
    );
}

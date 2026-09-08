//! Named parameters: a statement built once and rendered with different values.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{param, Postgres, Sqlite};

#[test]
fn a_bound_parameter_renders_as_a_placeholder_carrying_its_value() {
    let query = User::query()
        .filter(User::age.ge(param::<i32>("min_age")))
        .bind("min_age", 18);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."age" >= $1"#,
        &[int(18)],
    );
}

#[test]
fn the_same_statement_renders_again_with_a_different_value() {
    let template = User::query().filter(User::name.eq(param::<String>("who")));
    assert_sql(
        template.clone().bind("who", "ada"),
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."name" = $1"#,
        &[text("ada")],
    );
    assert_sql(
        template.bind("who", "grace"),
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."name" = $1"#,
        &[text("grace")],
    );
}

#[test]
fn binding_a_name_twice_keeps_the_last_value() {
    let query = User::query()
        .filter(User::age.ge(param::<i32>("min_age")))
        .bind("min_age", 18)
        .bind("min_age", 21);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."age" >= $1"#,
        &[int(21)],
    );
}

#[test]
fn a_parameter_left_unbound_is_refused_rather_than_rendered_empty() {
    let query = User::query().filter(User::age.ge(param::<i32>("min_age")));
    assert_rejected(query, &Postgres, "min_age");
}

#[test]
fn every_use_of_one_name_binds_the_value_again_in_placeholder_order() {
    let query = User::query()
        .filter(User::name.eq(param::<String>("who")))
        .filter(User::email.eq(param::<String>("who")))
        .bind("who", "ada");
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE "users"."name" = $1 AND "users"."email" = $2"#,
        &[text("ada"), text("ada")],
    );
}

#[test]
fn a_parameter_inside_a_subquery_resolves_from_the_outer_statement() {
    let recent = Post::query()
        .correlate::<User>()
        .filter(Post::user_id.eq(User::id))
        .filter(Post::views.ge(param::<i64>("floor")));
    let query = User::query().filter(exists(recent)).bind("floor", 100);
    assert_sql(
        query,
        &Postgres,
        r#"SELECT * FROM "users" WHERE EXISTS (SELECT * FROM "posts" WHERE "posts"."user_id" = "users"."id" AND "posts"."views" >= $1)"#,
        &[int(100)],
    );
}

#[test]
fn the_write_statements_bind_named_parameters_too() {
    let update = User::update()
        .set(User::name, param::<String>("new_name"))
        .filter(User::id.eq(param::<i64>("id")))
        .bind("new_name", "ada")
        .bind("id", 7);
    assert_sql(
        update,
        &Postgres,
        r#"UPDATE "users" SET "name" = $1 WHERE "users"."id" = $2"#,
        &[text("ada"), int(7)],
    );

    let delete = User::delete()
        .filter(User::id.eq(param::<i64>("id")))
        .bind("id", 7);
    assert_sql(
        delete,
        &Sqlite,
        r#"DELETE FROM "users" WHERE "users"."id" = ?"#,
        &[int(7)],
    );
}

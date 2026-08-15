//! INSERT / UPDATE / DELETE rendering (Phase 4b).

use oxider_query::prelude::*;
use oxider_query::Value;

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
    active: bool,
}

#[test]
fn insert_renders_columns_and_placeholders() {
    let r = User::insert()
        .value(User::name, "Alice")
        .value(User::age, 30)
        .value(User::active, true)
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"INSERT INTO "users" ("name", "age", "active") VALUES ($1, $2, $3)"#
    );
    assert_eq!(
        r.params,
        vec![
            Value::Text("Alice".into()),
            Value::Int(30),
            Value::Bool(true)
        ]
    );
}

#[test]
fn update_binds_set_values_before_where() {
    let r = User::update()
        .set(User::name, "Bob")
        .set(User::age, 41)
        .filter(User::id.eq(7))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"UPDATE "users" SET "name" = $1, "age" = $2 WHERE ("users"."id" = $3)"#
    );
    assert_eq!(
        r.params,
        vec![Value::Text("Bob".into()), Value::Int(41), Value::Int(7)]
    );
}

#[test]
fn delete_with_filter() {
    let r = User::delete()
        .filter(User::active.eq(false))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"DELETE FROM "users" WHERE ("users"."active" = $1)"#
    );
    assert_eq!(r.params, vec![Value::Bool(false)]);
}

#[test]
fn delete_without_filter_targets_whole_table() {
    let r = User::delete().render(&Postgres);
    assert_eq!(r.sql, r#"DELETE FROM "users""#);
    assert!(r.params.is_empty());
}

#[test]
fn update_renders_mysql_placeholders() {
    let r = User::update()
        .set(User::name, "Bob")
        .filter(User::id.eq(7))
        .render(&MySql);

    assert_eq!(
        r.sql,
        "UPDATE `users` SET `name` = ? WHERE (`users`.`id` = ?)"
    );
}

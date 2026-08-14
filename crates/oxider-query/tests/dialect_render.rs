//! The same query AST must render correctly across all supported dialects.

use oxider_query::prelude::*;
use oxider_query::{SelectQuery, Value};

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
}

/// A single query built once, then rendered per dialect.
fn built() -> SelectQuery {
    User::query()
        .select((User::id, User::name))
        .filter(User::name.eq("Alice").and(User::age.gt(10)))
        .build()
}

#[test]
fn postgres_numbered_placeholders_and_double_quotes() {
    let r = built().render(&Postgres);
    assert_eq!(
        r.sql,
        r#"SELECT "users"."id", "users"."name" FROM "users" WHERE (("users"."name" = $1) AND ("users"."age" > $2))"#
    );
    assert_eq!(r.params, vec![Value::Text("Alice".into()), Value::Int(10)]);
}

#[test]
fn mysql_positional_placeholders_and_backticks() {
    let r = built().render(&MySql);
    assert_eq!(
        r.sql,
        "SELECT `users`.`id`, `users`.`name` FROM `users` WHERE ((`users`.`name` = ?) AND (`users`.`age` > ?))"
    );
    assert_eq!(r.params, vec![Value::Text("Alice".into()), Value::Int(10)]);
}

#[test]
fn sqlite_positional_placeholders_and_double_quotes() {
    let r = built().render(&Sqlite);
    assert_eq!(
        r.sql,
        r#"SELECT "users"."id", "users"."name" FROM "users" WHERE (("users"."name" = ?) AND ("users"."age" > ?))"#
    );
}

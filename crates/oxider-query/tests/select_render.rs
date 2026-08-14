//! End-to-end tests: derive an entity, build a query, render Postgres SQL.

use oxider_query::prelude::*;
use oxider_query::Value;

// Fields are read only through the generated metamodel, not the struct itself.
#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
}

#[test]
fn renders_select_with_where() {
    let u = User::table();
    let rendered = Query::select()
        .from(u)
        .select((u.id, u.name))
        .filter(u.name.eq("Alice"))
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id", "users"."name" FROM "users" WHERE ("users"."name" = $1)"#
    );
    assert_eq!(rendered.params, vec![Value::Text("Alice".into())]);
}

#[test]
fn combines_predicates_with_and() {
    let u = User::table();
    let rendered = Query::select()
        .from(u)
        .select(u.id)
        .filter(u.name.eq("Bob").and(u.id.gt(10)))
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id" FROM "users" WHERE (("users"."name" = $1) AND ("users"."id" > $2))"#
    );
    assert_eq!(
        rendered.params,
        vec![Value::Text("Bob".into()), Value::Int(10)]
    );
}

#[test]
fn nullable_flag_tracked_from_option() {
    let u = User::table();
    assert!(!u.id.nullable);
    assert!(u.email.nullable);
}

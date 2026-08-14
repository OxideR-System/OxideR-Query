//! End-to-end tests: derive an entity, build a query, render Postgres SQL.

use oxider_query::prelude::*;
use oxider_query::Value;

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
    email: Option<String>,
}

#[test]
fn renders_select_with_where() {
    let rendered = User::query()
        .select((User::id, User::name))
        .filter(User::name.eq("Alice"))
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id", "users"."name" FROM "users" WHERE ("users"."name" = $1)"#
    );
    assert_eq!(rendered.params, vec![Value::Text("Alice".into())]);
}

#[test]
fn combines_predicates_with_and() {
    let rendered = User::query()
        .select(User::id)
        .filter(User::name.eq("Bob").and(User::age.gt(10)))
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id" FROM "users" WHERE (("users"."name" = $1) AND ("users"."age" > $2))"#
    );
    assert_eq!(
        rendered.params,
        vec![Value::Text("Bob".into()), Value::Int(10)]
    );
}

#[test]
fn order_limit_offset() {
    let rendered = User::query()
        .select(User::id)
        .order_by(User::age.desc())
        .order_by(User::name.asc())
        .limit(20)
        .offset(40)
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id" FROM "users" ORDER BY "users"."age" DESC, "users"."name" ASC LIMIT 20 OFFSET 40"#
    );
}

#[test]
fn contains_renders_like_with_wrapped_param() {
    let rendered = User::query()
        .select(User::id)
        .filter(User::name.contains("nguyen"))
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id" FROM "users" WHERE ("users"."name" LIKE $1)"#
    );
    assert_eq!(rendered.params, vec![Value::Text("%nguyen%".into())]);
}

#[test]
fn filter_opt_skips_none_and_ands_some() {
    let name: Option<String> = None;
    let min_age: Option<i32> = Some(18);

    let rendered = User::query()
        .select(User::id)
        .filter_opt(name.map(|v| User::name.eq(v)))
        .filter_opt(min_age.map(|v| User::age.ge(v)))
        .render(&Postgres);

    assert_eq!(
        rendered.sql,
        r#"SELECT "users"."id" FROM "users" WHERE ("users"."age" >= $1)"#
    );
    assert_eq!(rendered.params, vec![Value::Int(18)]);
}

#[test]
fn nullable_flag_tracked_from_option() {
    // Bind to locals so clippy does not treat these as constant assertions.
    let id_nullable = User::id.nullable;
    let email_nullable = User::email.nullable;
    assert!(!id_nullable);
    assert!(email_nullable);
}

//! Subquery rendering: IN / NOT IN and EXISTS / NOT EXISTS (Phase 4c).

use oxider_query::prelude::*;
use oxider_query::Value;

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    department_id: i64,
}

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "departments")]
struct Department {
    id: i64,
    name: String,
    active: bool,
}

#[test]
fn in_subquery_renders_and_orders_params() {
    let active_departments = Department::query()
        .filter(Department::active.eq(true))
        .scalar(Department::id);

    let r = User::query()
        .filter(User::department_id.in_subquery(active_departments))
        .select(User::name)
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "users"."name" FROM "users" WHERE ("users"."department_id" IN (SELECT "departments"."id" FROM "departments" WHERE ("departments"."active" = $1)))"#
    );
    assert_eq!(r.params, vec![Value::Bool(true)]);
}

#[test]
fn not_in_subquery_renders() {
    let named = Department::query()
        .filter(Department::name.eq("Legacy"))
        .scalar(Department::id);

    let r = User::query()
        .filter(User::department_id.not_in_subquery(named))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT * FROM "users" WHERE ("users"."department_id" NOT IN (SELECT "departments"."id" FROM "departments" WHERE ("departments"."name" = $1)))"#
    );
    assert_eq!(r.params, vec![Value::Text("Legacy".into())]);
}

#[test]
fn exists_and_outer_filter_param_order() {
    let sub = Department::query().filter(Department::active.eq(true));

    let r = User::query()
        .filter(exists(sub).and(User::name.eq("Alice")))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT * FROM "users" WHERE (EXISTS (SELECT * FROM "departments" WHERE ("departments"."active" = $1)) AND ("users"."name" = $2))"#
    );
    assert_eq!(
        r.params,
        vec![Value::Bool(true), Value::Text("Alice".into())]
    );
}

#[test]
fn not_exists_renders() {
    let sub = Department::query().filter(Department::name.eq("AI"));
    let r = User::query().filter(not_exists(sub)).render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT * FROM "users" WHERE NOT EXISTS (SELECT * FROM "departments" WHERE ("departments"."name" = $1))"#
    );
    assert_eq!(r.params, vec![Value::Text("AI".into())]);
}

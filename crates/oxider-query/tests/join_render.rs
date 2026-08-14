//! JOIN rendering and multi-entity selection (Phase 3a).

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
fn inner_join_selects_from_both_entities() {
    let r = User::query()
        .join::<Department>(User::department_id.eq_column(Department::id))
        .select((User::name, Department::name))
        .filter(Department::name.eq("AI"))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "users"."name", "departments"."name" FROM "users" INNER JOIN "departments" ON ("users"."department_id" = "departments"."id") WHERE ("departments"."name" = $1)"#
    );
    assert_eq!(r.params, vec![Value::Text("AI".into())]);
}

#[test]
fn left_join_renders() {
    let r = User::query()
        .left_join::<Department>(User::department_id.eq_column(Department::id))
        .select(User::name)
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "users"."name" FROM "users" LEFT JOIN "departments" ON ("users"."department_id" = "departments"."id")"#
    );
}

#[test]
fn join_condition_params_precede_where_params() {
    // A literal in the ON clause must bind before the WHERE parameter.
    let r = User::query()
        .join::<Department>(
            User::department_id
                .eq_column(Department::id)
                .and(Department::active.eq(true)),
        )
        .select(User::name)
        .filter(User::name.eq("Alice"))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "users"."name" FROM "users" INNER JOIN "departments" ON (("users"."department_id" = "departments"."id") AND ("departments"."active" = $1)) WHERE ("users"."name" = $2)"#
    );
    assert_eq!(
        r.params,
        vec![Value::Bool(true), Value::Text("Alice".into())]
    );
}

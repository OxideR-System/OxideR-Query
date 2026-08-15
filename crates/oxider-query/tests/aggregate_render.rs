//! Aggregate, GROUP BY, and HAVING rendering (Phase 4).

use oxider_query::prelude::*;
use oxider_query::Value;

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "orders")]
struct Order {
    id: i64,
    customer_id: i64,
    total: f64,
    status: String,
}

#[test]
fn count_all_with_group_by() {
    let r = Order::query()
        .select((Order::customer_id, count_all()))
        .group_by(Order::customer_id)
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "orders"."customer_id", COUNT(*) FROM "orders" GROUP BY "orders"."customer_id""#
    );
    assert!(r.params.is_empty());
}

#[test]
fn sum_avg_min_max_render() {
    let r = Order::query()
        .select((
            sum(Order::total),
            avg(Order::total),
            min(Order::total),
            max(Order::total),
        ))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT SUM("orders"."total"), AVG("orders"."total"), MIN("orders"."total"), MAX("orders"."total") FROM "orders""#
    );
}

#[test]
fn group_by_with_having_binds_param() {
    let r = Order::query()
        .select((Order::customer_id, count_all()))
        .filter(Order::status.eq("paid"))
        .group_by(Order::customer_id)
        .having(count_all().gt(5))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "orders"."customer_id", COUNT(*) FROM "orders" WHERE ("orders"."status" = $1) GROUP BY "orders"."customer_id" HAVING (COUNT(*) > $2)"#
    );
    assert_eq!(r.params, vec![Value::Text("paid".into()), Value::Int(5)]);
}

#[test]
fn count_column_and_having_on_sum() {
    let r = Order::query()
        .select((Order::customer_id, count(Order::id)))
        .group_by(Order::customer_id)
        .having(sum(Order::total).ge(100.0))
        .render(&Postgres);

    assert_eq!(
        r.sql,
        r#"SELECT "orders"."customer_id", COUNT("orders"."id") FROM "orders" GROUP BY "orders"."customer_id" HAVING (SUM("orders"."total") >= $1)"#
    );
    assert_eq!(r.params, vec![Value::Real(100.0)]);
}

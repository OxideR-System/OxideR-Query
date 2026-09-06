//! Render-throughput benchmarks: measure building and rendering queries to SQL.
//!
//! These measure OxideR-Query's own AST-to-SQL path, not a comparison against
//! other libraries. Building and rendering are timed together because that is
//! what an application does per request: the builder is consumed by the render,
//! so there is no cached statement to amortise the cost against.
//!
//! Run with `cargo bench -p oxider-query`.

use criterion::{criterion_group, criterion_main, Criterion};
use oxider_query::prelude::*;
use std::hint::black_box;

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
    age: i32,
    active: bool,
    department_id: i64,
}

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "departments")]
struct Department {
    id: i64,
    name: String,
    budget: f64,
}

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "orders")]
struct Order {
    id: i64,
    user_id: i64,
    total: f64,
    status: String,
}

fn bench_render(c: &mut Criterion) {
    c.bench_function("simple_select", |b| {
        b.iter(|| {
            User::query()
                .select((User::id, User::name))
                .filter(User::age.ge(18))
                .to_sql(black_box(&Postgres))
        })
    });

    c.bench_function("join_group_having", |b| {
        b.iter(|| {
            User::query()
                .inner_join(Department::table(), User::department_id.eq(Department::id))
                .select((Department::name, count_all()))
                .filter(User::age.ge(18))
                .group_by(Department::name)
                .having(count_all().gt(5))
                .order_by(Department::name.asc())
                .to_sql(black_box(&Postgres))
        })
    });

    // A wide predicate tree: the precedence ladder decides parenthesisation at
    // every node, so this is the path that grows with expression depth.
    c.bench_function("deep_predicate", |b| {
        b.iter(|| {
            User::query()
                .select(User::id)
                .filter(
                    User::age
                        .ge(18)
                        .and(User::age.lt(65))
                        .and(User::name.starts_with("a").or(User::name.ends_with("z")))
                        .and(User::email.is_not_null())
                        .and(User::active.eq(true).or(User::department_id.gt(10))),
                )
                .to_sql(black_box(&Postgres))
        })
    });

    // A correlated subquery plus a window function: the two constructs whose
    // rendering recurses into a nested serializer state.
    c.bench_function("correlated_subquery_and_window", |b| {
        b.iter(|| {
            let spend = Order::query()
                .correlate::<User>()
                .filter(Order::user_id.eq(User::id))
                .scalar(Order::total.sum());
            User::query()
                .select((
                    User::name,
                    spend,
                    row_number().over(Window::new().partition_by(User::department_id)),
                ))
                .to_sql(black_box(&Postgres))
        })
    });

    // The same statement rendered by each dialect, to show the cost is in the
    // walk rather than in any one dialect's templates.
    let mut dialects = c.benchmark_group("dialects");
    for (name, dialect) in [
        ("postgres", &Postgres as &dyn Dialect),
        ("mysql", &MySql),
        ("sqlite", &Sqlite),
    ] {
        dialects.bench_function(name, |b| {
            b.iter(|| {
                User::query()
                    .inner_join(Order::table(), Order::user_id.eq(User::id))
                    .select((User::name, Order::total.sum()))
                    .filter(User::active.eq(true).and(Order::status.eq("paid")))
                    .group_by(User::name)
                    .to_sql(black_box(dialect))
            })
        });
    }
    dialects.finish();

    c.bench_function("multi_row_insert", |b| {
        b.iter(|| {
            User::insert()
                .columns((User::name, User::age, User::active))
                .values(("ada", 36, true))
                .values(("grace", 45, true))
                .values(("alan", 41, false))
                .to_sql(black_box(&Postgres))
        })
    });
}

criterion_group!(benches, bench_render);
criterion_main!(benches);

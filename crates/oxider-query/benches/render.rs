//! Render-throughput benchmarks: measure building and rendering queries to SQL.
//!
//! These measure OxideR-Query's own AST-to-SQL path, not a comparison against
//! other libraries. Run with `cargo bench -p oxider-query`.

use criterion::{criterion_group, criterion_main, Criterion};
use oxider_query::prelude::*;
use std::hint::black_box;

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
    department_id: i64,
}

#[allow(dead_code)]
#[derive(Entity)]
#[oxider(table = "departments")]
struct Department {
    id: i64,
    name: String,
}

fn bench_render(c: &mut Criterion) {
    c.bench_function("simple_select", |b| {
        b.iter(|| {
            User::query()
                .select((User::id, User::name))
                .filter(User::age.ge(18))
                .render(black_box(&Postgres))
        })
    });

    c.bench_function("join_group_having", |b| {
        b.iter(|| {
            User::query()
                .join::<Department>(User::department_id.eq_column(Department::id))
                .select((Department::name, count_all()))
                .filter(User::age.ge(18))
                .group_by(Department::name)
                .having(count_all().gt(5))
                .order_by(User::age.desc())
                .render(black_box(&Postgres))
        })
    });
}

criterion_group!(benches, bench_render);
criterion_main!(benches);

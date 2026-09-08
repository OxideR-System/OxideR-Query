//! Queries built in a loop, which is how a query gets deep by accident.
//!
//! The renderer walks the AST recursively, so depth is a real limit rather
//! than a theoretical one: past a few thousand levels the walk used to
//! overflow the stack, which aborts the process instead of raising something a
//! caller can handle. Two things keep that from happening, and both are pinned
//! here - `AND`/`OR` chains are flattened so accumulating filters does not add
//! depth at all, and anything that is genuinely nested is refused with
//! [`RenderError::TooDeep`] before the stack runs out.

mod common;

use common::*;
use oxider_query::prelude::*;
use oxider_query::{Postgres, RenderError, Renderable, MAX_DEPTH};

/// Enough conditions that a nesting chain would have overflowed the stack.
const MANY: usize = 5_000;

#[test]
fn a_filter_accumulated_in_a_loop_stays_flat_and_renders() {
    let mut query = User::query().select(User::id);
    for id in 0..MANY {
        query = query.filter(User::id.ne(id as i64));
    }

    let rendered = render(query, &Postgres);

    assert_eq!(rendered.params.len(), MANY);
    assert!(rendered.sql.starts_with(
        r#"SELECT "users"."id" FROM "users" WHERE "users"."id" <> $1 AND "users"."id" <> $2"#
    ));
    // Flattened, so no parenthesis is introduced anywhere by the nesting.
    assert!(!rendered.sql.contains('('));
}

#[test]
fn a_flattened_chain_renders_exactly_as_the_nested_one_did() {
    // Three conditions, one clause: the shape changed, the SQL did not.
    assert_sql(
        User::query()
            .select(User::id)
            .filter(User::age.ge(18))
            .filter(User::active.eq(true))
            .filter(User::name.eq("ada")),
        &Postgres,
        r#"SELECT "users"."id" FROM "users" WHERE "users"."age" >= $1 AND "users"."active" = $2 AND "users"."name" = $3"#,
        &[int(18), flag(true), text("ada")],
    );
}

#[test]
fn mixing_and_with_or_still_groups_by_precedence() {
    assert_sql(
        User::query().select(User::id).filter(
            User::age
                .ge(18)
                .and(User::active.eq(true))
                .or(User::name.eq("ada")),
        ),
        &Postgres,
        r#"SELECT "users"."id" FROM "users" WHERE "users"."age" >= $1 AND "users"."active" = $2 OR "users"."name" = $3"#,
        &[int(18), flag(true), text("ada")],
    );
    assert_sql(
        User::query().select(User::id).filter(
            User::name
                .eq("ada")
                .or(User::age.ge(18).and(User::active.eq(true))),
        ),
        &Postgres,
        r#"SELECT "users"."id" FROM "users" WHERE "users"."name" = $1 OR "users"."age" >= $2 AND "users"."active" = $3"#,
        &[text("ada"), int(18), flag(true)],
    );
}

#[test]
fn a_genuinely_nested_expression_is_refused_rather_than_overflowing_the_stack() {
    let mut expression = User::age.into_expr();
    for _ in 0..MANY {
        expression = expression.add(1i32);
    }

    let error = User::query()
        .select(expression)
        .render_with(&Postgres)
        .expect_err("a chain this deep must be refused");

    assert_eq!(error, RenderError::TooDeep { limit: MAX_DEPTH });
    assert!(error.to_string().contains("nests more than"));
}

#[test]
fn nesting_a_query_would_actually_use_renders_normally() {
    // Twenty levels: far past anything hand-written, far short of the limit.
    let mut expression = User::age.into_expr();
    for _ in 0..20 {
        expression = expression.add(1i32);
    }
    assert!(User::query()
        .select(expression)
        .render_with(&Postgres)
        .is_ok());
}

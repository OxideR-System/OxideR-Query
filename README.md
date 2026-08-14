# OxideR-Query

Type-safe, multi-dialect SQL query builder for Rust, inspired by Java's [QueryDSL](https://github.com/querydsl/querydsl) but pushing type-safety further with Rust's type system.

> Status: 0.1.0, in active development. SELECT with typed WHERE, rendering to PostgreSQL, MySQL, and SQLite. Pre-1.0, so the API tracks latest stable Rust and may change.

## What it does today

Define an entity as a plain struct, derive `Entity`, then build a query whose column types are checked at compile time.

```rust
use oxider_query::prelude::*;

#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
}

let u = User::table();
let rendered = Query::select()
    .from(u)
    .select((u.id, u.name))
    .filter(u.name.eq("Alice"))
    .render(&Postgres);

// rendered.sql:
//   SELECT "users"."id", "users"."name" FROM "users" WHERE ("users"."name" = $1)
// rendered.params: ["Alice"]
```

The same query renders to any supported dialect - the AST is built once, the dialect only changes quoting and placeholder style:

| Dialect | Identifiers | Placeholders |
|---------|-------------|--------------|
| `Postgres` | `"ident"` | `$1, $2` |
| `MySql` | `` `ident` `` | `?` |
| `Sqlite` | `"ident"` | `?` |

Comparing a column against the wrong SQL type is a compile error:

```rust
u.name.eq(123);          // error: i64: IntoExpr<Text> is not satisfied
```

## Design

- **Query builder, layered.** The core produces `(sql, params)` and is database-agnostic. Connection handling and row mapping are a separate, optional layer added later.
- **AST separated from rendering.** Queries build a dialect-agnostic AST; `render(&dialect)` emits dialect-specific SQL. This is the key to multi-dialect support without duplicating logic.
- **Selective type-state.** Type-safety is enforced where it matters (SQL type of comparisons now; joined-table scoping and nullability later), without type-stating everything into unreadable errors.

## Workspace

| Crate | Role |
|-------|------|
| `oxider-query-core` | AST, typed expression layer, builder, `Dialect` trait, renderer. No DB, no macros. |
| `oxider-query-macros` | `#[derive(Entity)]` generating the query metamodel. |
| `oxider-query` | Facade crate that downstream users depend on. |

## Roadmap

See `plans/260814-1626-oxider-query-architecture-roadmap/plan.md`. Next up: joins with type-tracked tables and nullability, then the remaining SQL clauses (ORDER BY, GROUP BY, aggregates, INSERT/UPDATE/DELETE).

## Development

```bash
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

## License

MIT

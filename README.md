# OxideR-Query

Type-safe, multi-dialect SQL query builder for Rust, inspired by Java's [QueryDSL](https://github.com/querydsl/querydsl) but pushing type-safety further with Rust's type system.

> Status: 0.1.0, in active development. Type-safe SELECT (WHERE, JOIN, aggregates, GROUP BY/HAVING, subqueries), INSERT/UPDATE/DELETE, rendering to PostgreSQL, MySQL, and SQLite, plus optional async execution over sqlx. Pre-1.0, so the API tracks latest stable Rust and may change.

## What it does today

Define an entity as a plain struct, derive `Entity`, then build a query whose column types are checked at compile time.

```rust
use oxider_query::prelude::*;

#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i32,
    email: Option<String>,
}

let rendered = User::query()
    .select((User::id, User::name))
    .filter(User::name.contains("nguyen").and(User::age.ge(18)))
    .order_by(User::age.desc())
    .limit(20)
    .render(&Postgres);

// rendered.sql:
//   SELECT "users"."id", "users"."name" FROM "users"
//   WHERE (("users"."name" LIKE $1) AND ("users"."age" >= $2))
//   ORDER BY "users"."age" DESC LIMIT 20
// rendered.params: ["%nguyen%", 18]
```

Columns are generated as associated consts (`User::id`), so there is no separate metamodel type to name - closer to QueryDSL's `QUser.user.id`, but terser. Build dynamic queries with `filter_opt`:

```rust
let users = User::query()
    .filter_opt(filter.name.map(|v| User::name.contains(v)))
    .filter_opt(filter.min_age.map(|v| User::age.ge(v)))
    .render(&Postgres);
```

Join across entities; the join key is type-checked (`eq_column` requires both columns to share a Rust type, so joining an `i64` key to a `String` column is a compile error):

```rust
let rows = User::query()
    .join::<Department>(User::department_id.eq_column(Department::id))
    .select((User::name, Department::name))
    .filter(Department::name.eq("AI"))
    .render(&Postgres);

// SELECT "users"."name", "departments"."name" FROM "users"
// INNER JOIN "departments" ON ("users"."department_id" = "departments"."id")
// WHERE ("departments"."name" = $1)
```

Aggregate, group, and filter groups. `SUM`/`AVG` accept only numeric columns; `MIN`/`MAX` only orderable ones:

```rust
let rows = Order::query()
    .select((Order::customer_id, count_all()))
    .filter(Order::status.eq("paid"))
    .group_by(Order::customer_id)
    .having(count_all().gt(5))
    .render(&Postgres);

// SELECT "orders"."customer_id", COUNT(*) FROM "orders"
// WHERE ("orders"."status" = $1)
// GROUP BY "orders"."customer_id" HAVING (COUNT(*) > $2)
```

INSERT, UPDATE, and DELETE share the same typed columns and dialect rendering. Assignments and predicates are type-checked to the target table:

```rust
User::insert().value(User::name, "Alice").value(User::age, 30).render(&Postgres);
// INSERT INTO "users" ("name", "age") VALUES ($1, $2)

User::update().set(User::name, "Bob").filter(User::id.eq(7)).render(&Postgres);
// UPDATE "users" SET "name" = $1 WHERE ("users"."id" = $2)

User::delete().filter(User::active.eq(false)).render(&Postgres);
// DELETE FROM "users" WHERE ("users"."active" = $1)
```

Subqueries: `IN`/`NOT IN` against a single-column subquery (type-matched to the outer column) and `EXISTS`/`NOT EXISTS`:

```rust
let active = Department::query().filter(Department::active.eq(true)).scalar(Department::id);
User::query().filter(User::department_id.in_subquery(active)).render(&Postgres);
// SELECT * FROM "users"
// WHERE ("users"."department_id" IN (SELECT "departments"."id" FROM "departments" WHERE ("departments"."active" = $1)))
```

The same query renders to any supported dialect - the AST is built once, the dialect only changes quoting and placeholder style:

| Dialect | Identifiers | Placeholders |
|---------|-------------|--------------|
| `Postgres` | `"ident"` | `$1, $2` |
| `MySql` | `` `ident` `` | `?` |
| `Sqlite` | `"ident"` | `?` |

Misuse is a compile error, with readable messages:

```rust
User::name.eq(123);        // error: the trait bound `i64: Into<String>` is not satisfied
User::age.contains("50");  // error: no method named `contains` found for Column<_, i32>
User::flag.gt(true);       // error: `bool: Orderable` is not satisfied

// Referencing a table you never joined is a compile error, too:
User::query().filter(Department::name.eq("AI"));
// error: the trait bound `Nil: Contains<Department, _>` is not satisfied
```

Every query tracks its in-scope tables (the FROM entity plus each join) at the type level, so `filter`, `select`, and `order_by` only accept columns of tables that are actually in scope. `join` extends that set.

## Running queries

The core builder is execution-agnostic; the optional `oxider-query-exec` crate provides one `Db` handle that wraps a sqlx pool for any backend, takes a query, renders it with the backend's dialect, and runs it over [sqlx](https://github.com/launchbadge/sqlx). Rows map into any `sqlx::FromRow` type. The dialect is chosen from the handle, so call sites pass the query directly - no `.render(&Sqlite)` - and switching databases is a one-line change of the handle's type, touching no query code. SQLite is implemented today (Postgres and MySQL will follow the same shape behind features):

```rust
#[derive(Entity, sqlx::FromRow)]
#[oxider(table = "users")]
struct User { id: i64, name: String, age: i64, active: bool }

let db = SqliteDb::connect("sqlite::memory:").await?;

db.execute(User::insert().value(User::name, "Alice").value(User::age, 30)).await?;

let adults: Vec<User> = db
    .fetch_all(User::query().filter(User::age.ge(18)).order_by(User::age.asc()))
    .await?;
```

## Design

- **Two ways to a metamodel.** Hand-write structs and `#[derive(Entity)]`, or point `oxider-query-codegen` at an existing database to generate those structs from its schema. Both produce the same `Entity` shape.
- **Query builder, layered.** The core produces `(sql, params)` and is database-agnostic. Connection handling and row mapping live in the separate, optional `oxider-query-exec` crate.
- **AST separated from rendering.** Queries build a dialect-agnostic AST; `render(&dialect)` emits dialect-specific SQL. This is the key to multi-dialect support without duplicating logic.
- **`Column<Entity, Type>`.** Each column carries its owning entity and Rust type as compile-time markers. The entity keeps column references and join keys type-checked; the Rust type gates operators (ordering only on orderable types, `LIKE` only on strings) and drives value binding.
- **Type-level table sets.** `Select<S>` tracks the in-scope entities as a type-level list; predicates and orderings carry the entities they reference, and clause methods require those to be in scope. Referencing a non-joined table fails to compile.
- **DX over maximal type-state.** Type-safety is enforced where it matters, but operators live as inherent methods so mistakes surface as plain "method not found" / "trait bound not satisfied" errors rather than Diesel-style walls.

## Workspace

| Crate | Role |
|-------|------|
| `oxider-query-core` | AST, typed expression layer, builder, `Dialect` trait, renderer. No DB, no macros. |
| `oxider-query-macros` | `#[derive(Entity)]` generating the query metamodel. |
| `oxider-query-exec` | Optional async execution over sqlx (SQLite today). Binds params, maps rows. |
| `oxider-query-codegen` | Optional schema introspection: generate `Entity` structs from an existing database (SQLite today). |
| `oxider-query` | Facade crate that downstream users depend on. |

## Roadmap

See `plans/260814-1626-oxider-query-architecture-roadmap/plan.md`. The core builder is feature-complete for SQLite: type-safe SELECT (WHERE, JOIN, aggregates, GROUP BY/HAVING, subqueries) and INSERT/UPDATE/DELETE, rendering to all three dialects, with async execution and schema codegen over SQLite. Remaining work: Postgres/MySQL execution and codegen backends (the builder is already multi-dialect, so each is param binding plus row typing), type-level nullability for outer joins (deferred until the row-mapping layer gives it a consumer), and correlated subqueries.

## Development

```bash
cargo test --all
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo bench -p oxider-query   # render-throughput microbench (criterion)
```

## License

MIT

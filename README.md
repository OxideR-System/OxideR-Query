# OxideR-Query

Type-safe, multi-dialect SQL query builder for Rust, inspired by Java's [QueryDSL](https://github.com/querydsl/querydsl) but pushing type-safety further than a JVM can.

> Status: 0.1.0, in active development. The full SELECT surface (every join, aliasing, `DISTINCT ON`, null ordering, row locking, set operations, CTEs including recursive ones, window functions, correlated subqueries), full DML (multi-row insert, insert-select, upsert, `RETURNING`, update-from, delete-using), roughly 200 operators rendered for PostgreSQL, MySQL and SQLite, plus optional async execution and schema codegen over SQLite. Pre-1.0, so the API tracks latest stable Rust and may change.

The user guide lives in [`docs/`](./docs/README.md), in Vietnamese, and is also published as a [Docusaurus site](./website).

## What it does

Define an entity as a plain struct, derive `Entity`, then build a query the compiler checks.

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
    .order_by(User::id.desc())
    .limit(20)
    .to_sql(&Postgres)?;

// rendered.sql:
//   SELECT "users"."id", "users"."name" FROM "users"
//   WHERE "users"."name" LIKE $1 ESCAPE '!' AND "users"."age" >= $2
//   ORDER BY "users"."id" DESC LIMIT 20
// rendered.params: ["%nguyen%", 18]
```

Two things in that output are deliberate.

`LIKE` carries an explicit `ESCAPE` clause and the value is escaped before the wildcards go on, so searching for `50%` does not match `500 units`. QueryDSL escapes only constant operands and silently skips dynamic ones; this port fixes that rather than reproducing it.

`to_sql` returns a `Result`. A dialect that cannot express what you built refuses it here, rather than emitting plausible-but-wrong SQL for the database to reject later.

Columns are associated consts (`User::id`), so there is no generated `QUser` type to import. Parentheses come from a central precedence ladder, not from each operator, so the SQL reads the way a person would write it.

### Dynamic queries

```rust
let query = User::query()
    .filter_opt(filter.name.map(|v| User::name.contains(v)))
    .filter_opt(filter.min_age.map(|v| User::age.ge(v)));
```

### Joins, checked at the type level

```rust
User::query()
    .inner_join(Department::table(), User::department_id.eq(Department::id))
    .select((User::name, Department::name))
    .filter(Department::name.eq("AI"));

// SELECT "users"."name", "departments"."name" FROM "users"
// INNER JOIN "departments" ON "users"."department_id" = "departments"."id"
// WHERE "departments"."name" = $1
```

A self-join gets an alias, and the aliased copy is a distinct entity at the type level, so both halves stay unambiguous:

```rust
User::query()
    .inner_join(
        User::table().alias("manager"),
        User::id.at("manager").eq(User::manager_id),
    )
    .select((User::name, User::name.at("manager")));
```

### Aggregates, windows, CTEs

```rust
User::query()
    .left_join(Order::table(), Order::user_id.eq(User::id))
    .select((User::name, Order::total.sum().alias("lifetime_value")))
    .group_by(User::name)
    .having(Order::total.sum().gt(1000.0));

let rank = row_number().over(
    Window::new().partition_by(Post::user_id).order_by(Post::views.desc()),
);
Post::query().select((Post::title, rank.alias("position")));
```

### DML

```rust
User::insert().set(User::name, "Alice").set(User::age, 30);

User::insert()
    .set(User::email, "ada@example.com")
    .on_conflict(["email"])
    .do_update()
    .set(User::name, excluded(User::name))
    .end();

Post::update().set(Post::views, Post::views.add(1)).filter(Post::id.eq(42));

Post::delete().using(User::table()).filter(Post::user_id.eq(User::id));
```

### One query, three dialects

The AST is built once; the dialect decides quoting, placeholders, operator spelling, and what is refused.

| Dialect | Identifiers | Placeholders |
|---------|-------------|--------------|
| `Postgres` | `"ident"` | `$1, $2` |
| `MySql` | `` `ident` `` | `?` |
| `Sqlite` | `"ident"` | `?` |

Where engines disagree, the rule is: emulate when the result is identical, refuse when it is not. `NULLS LAST` and aggregate `FILTER` are emulated, and tests prove the emulation returns the same rows as the native clause on a real database. `STDDEV` on SQLite is refused, because faking it with arithmetic would return subtly wrong numbers.

### Misuse is a compile error

```rust
User::name.eq(123);
// error[E0277]: the trait bound `{integer}: IntoExpr<String>` is not satisfied

User::query().filter(Department::name.eq("AI"));
// error[E0277]: the trait bound `Nil: Contains<Department, _>` is not satisfied

Order::status.sum();
// error[E0599]: the method `sum` exists for `Column<Order, String>`,
//               but `String: Numeric` is not satisfied
```

Every query tracks its in-scope tables at the type level, so `filter`, `select` and `order_by` only accept columns of tables that are actually there. A correlated subquery additionally tracks the outer entities it is free in, so using one where the outer table is absent is also a compile error. Each guarantee has a `compile_fail` doc test; see [chapter 14](./docs/14-type-safety.md), which is also honest about what is *not* checked.

## Security model

**Values always bind, identifiers are always constants.** Anything you compare, assign or insert becomes a placeholder plus an entry in `params`. Anything that names a table, column, alias or schema has type `&'static str`, so it has to be a literal in your source or codegen output. A `String` built at runtime does not compile in an identifier position, which is the point:

```rust
let name = format!("t_{input}");
select_from_name(&name);
// error[E0597]: `name` does not live long enough
```

The single way around that is `String::leak`, which is a hole you open yourself. Match untrusted input against a whitelist of literals instead. Rendered identifiers are still dialect-quoted with inner quotes doubled, but that is the second line of defence, not the first.

`raw`, `RawBuilder` and `col` splice SQL verbatim. All of them take `&'static str`, so untrusted input cannot reach them, but they bypass the template layer entirely: treat their contents as source code and bind values with `RawBuilder::bind` rather than interpolating.

`LIMIT` and `OFFSET` are inlined rather than bound. They are `u64`, so there is nothing to inject.

Named parameters make a statement a template. `param::<T>("name")` places the hole, `bind("name", value)` fills it, and a name left unbound is `Err(RenderError::UnboundParameter)` rather than a query missing a condition:

```rust
let template = User::query().filter(User::age.ge(param::<i32>("min_age")));
template.clone().bind("min_age", 18).to_sql(&Postgres)?;
template.bind("min_age", 21).to_sql(&Postgres)?;
```

Chained `AND`/`OR` flatten into one many-operand node, so a thousand dynamic filters stay one level deep. Genuinely deep trees stop at `MAX_DEPTH` (256) with `Err(RenderError::TooDeep)` instead of overflowing the stack.

`oxider-query-codegen` binds the table name into its introspection query and escapes table and column names as Rust string literals in the generated source, so a hostile schema is a naming problem rather than a code-execution one. It is still a build-time tool: point it at a database you trust.

[Chapter 16](./docs/16-security-model.md) has the full trust boundary, with a table of who is responsible for what.

## Running queries

The core builder is execution-agnostic. The optional `oxider-query-exec` crate provides one `Db` handle wrapping a sqlx pool: it renders with the backend's own dialect, binds the parameters and runs the statement, mapping rows into any `sqlx::FromRow` type.

```rust
#[derive(Entity, sqlx::FromRow)]
#[oxider(table = "users")]
struct User { id: i64, name: String, age: i64, active: bool }

let db = SqliteDb::connect("sqlite::memory:").await?;

db.execute(User::insert().set(User::name, "Alice").set(User::age, 30)).await?;

let adults: Vec<User> = db
    .fetch_all(User::query().filter(User::age.ge(18)).order_by(User::age.asc()))
    .await?;
```

There is no `.to_sql(&Sqlite)` at the call site: the handle picks the dialect, so pointing at another database is a one-line change of the handle's type and touches no query code.

`db.transaction(async |tx| { ... })` scopes a transaction to a closure, committing on `Ok` and rolling back on `Err`, so a commit is never forgotten. `db.begin()` is the manual form.

`oxider_query_exec::Error` separates `Render` from `Database`, so a query the engine cannot express fails before a connection is touched.

## Design

- **Four layers.** `ast` (untyped trees plus the operator catalog), `dialect` (per-engine templates and `Caps`), `render` (one walker over `&dyn Dialect`), `typed`/`builder` (what users write, and everything the compiler checks). Adding an operator is a row in a table; adding a dialect is overriding a few rows.
- **Templates, not a match arm per operator.** QueryDSL's idea, ported: the renderer knows nothing about individual operators, it looks them up. Parenthesisation is decided once from a precedence ladder, exactly where QueryDSL decides it.
- **`Column<Entity, Type>`.** The entity marker drives the scope check; the Rust type gates operators and drives value binding.
- **Type-level table sets.** `Select<S, F>` tracks in-scope entities in `S` and the outer entities a correlated subquery is free in as `F`. Both are erased at compile time.
- **Nullability stays out of the type.** `Option<T>` fields set a runtime flag rather than widening the column type, because widening removes the string and numeric operators from exactly the columns that need them most. The trade-off is documented rather than hidden.
- **Two ways to a metamodel.** Hand-write structs and derive `Entity`, or point `oxider-query-codegen` at an existing database. Both produce the same shape.

## Workspace

| Crate | Role |
|-------|------|
| `oxider-query-core` | AST, typed expression layer, builders, `Dialect` trait, renderer. No DB, no macros. |
| `oxider-query-macros` | `#[derive(Entity)]` generating the query metamodel. |
| `oxider-query-exec` | Optional async execution over sqlx (SQLite today). Binds params, maps rows. |
| `oxider-query-codegen` | Optional schema introspection: generate `Entity` structs from an existing database (SQLite today). |
| `oxider-query` | Facade crate that downstream users depend on. |

## Documentation

The guide is in [`docs/`](./docs/README.md), sixteen chapters from installation through to the API reference and the security model.

The `website/` directory renders that same directory as a Docusaurus site, so there is one copy of the guide:

```bash
cd website
npm install
npm start        # dev server
npm run build    # static site
```

## Testing

Tests are scenarios rather than unit tests: each one builds a query a real application would build and pins the complete SQL string and the complete parameter list, for every dialect that can express it. Pinning the whole string is deliberate; a partial `contains` assertion would have missed the precedence, quoting and placeholder-numbering bugs these tests exist to catch.

| Suite | Covers |
|---|---|
| `crates/oxider-query/tests/` | SELECT, joins, expressions, aggregates, windows, subqueries, set operations and CTEs, DML, entity mapping, and every example printed in the guide |
| `crates/oxider-query-exec/tests/sqlite_end_to_end.rs` | the hard cases against a real database: escaped `LIKE` actually matching, emulated null ordering actually ordering, emulated `FILTER` counting the same rows as the native one, recursive CTEs, upserts, `RETURNING`, transaction rollback |
| `crates/oxider-query-codegen/tests/` | generated source parses as Rust, including keyword and non-identifier column names |
| `crates/oxider-query/tests/named_parameter_scenarios.rs` | named parameters: rebinding, an unbound name refused, resolution inside a correlated subquery |
| `crates/oxider-query/tests/dynamic_query_depth_scenarios.rs` | flattened `AND`/`OR` chains, and the depth limit refusing rather than overflowing |
| doc tests | each advertised compile-time guarantee, as `compile_fail` |

## Development

```bash
cargo test --workspace --all-targets --all-features
cargo test --workspace --all-features --doc
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo bench -p oxider-query   # render-throughput microbench (criterion)
```

## Roadmap

See `plans/260905-2058-querydsl-full-port/plan.md`.

Remaining: PostgreSQL and MySQL execution and codegen backends (the builder is already multi-dialect, so each is parameter binding plus row typing), `#[derive(Projection)]`, and a GroupBy transformer.

## License

MIT

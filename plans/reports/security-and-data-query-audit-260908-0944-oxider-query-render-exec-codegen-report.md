# OxideR-Query - Security / Transaction / Memory / Query Audit

Date: 2026-09-08
Scope: `crates/oxider-query-core`, `-exec`, `-codegen`, `-macros` (~12k LOC, 68 files).
Baseline health: `cargo clippy --workspace --all-targets --all-features` clean, `cargo test --workspace --all-features` 22/22 suites green, `#![forbid(unsafe_code)]` on every crate.
Every finding marked CONFIRMED was reproduced empirically in this session.

The audit sections below are the original write-up, kept as written except where a correction is called out inline.
Findings 1 to 16 were the audit; finding 17 was found while fixing finding 4.
All of them have since been fixed: see [Remediation](#remediation) for what changed, what was corrected, and what was left open.

## Summary

| # | Severity | Area | Finding | Status |
|---|----------|------|---------|--------|
| 1 | High | codegen | DB table/column names written verbatim into generated Rust source -> arbitrary code injection at build time | CONFIRMED |
| 2 | Medium | codegen | `PRAGMA table_info("{table}")` built by string interpolation, quote not escaped | CONFIRMED |
| 3 | Medium | core/render | `next_val`/`curr_val` splice the sequence name unescaped inside a `'...'` literal | CONFIRMED |
| 4 | Medium | core/render | Renderer is recursive -> deep predicate chain overflows the stack, aborts the process | CONFIRMED |
| 5 | Medium | core/render | Set-op branch keeps its own `WITH`/`ORDER BY`/`LIMIT` -> invalid SQL where branches are not wrapped (SQLite) | CONFIRMED |
| 6 | Medium | core/render | `UPDATE` with no assignments renders `SET` with nothing after it | CONFIRMED |
| 7 | Medium | core/dialect | `Update::from` / `Delete::using` have no capability gate -> invalid SQL on SQLite and MySQL | CONFIRMED (SQLite and MySQL 8.4) |
| 8 | Medium | exec | `Db::transaction` discards the caller's error when rollback itself fails | code path |
| 9 | Medium | core/builder | Silent no-ops: `skip_locked`/`no_wait` without a lock, `exclude` without a frame | CONFIRMED |
| 10 | Low-Med | core/builder | `Insert` silently drops values when `set`/`columns`/`from_query` are mixed | CONFIRMED |
| 11 | Low | core/typed | `param()` is public but nothing can ever bind it -> always fails at render | CONFIRMED |
| 12 | Low | core/typed | Empty `IN` renders as bare `1 = 0` keyword, unparenthesised | CONFIRMED |
| 13 | Low | core/dialect | `IndexOfFrom` emulation returns `start - 1` instead of `0` when the needle is absent | code path |
| 14 | Low | exec/core | Every parameter value is deep-cloned twice per execution | code path |
| 15 | Info | project | `&'static str` is the only defence against identifier injection, and `String::leak` defeats it | design |
| 16 | Info | CI | CI runs `cargo test --all` without `--all-features`; no `cargo audit`/`deny` | - |
| 17 | Medium | core/ast | `Node`'s derived `Drop` recurses -> deep tree aborts the process on free, after the render guard already refused it | CONFIRMED (found during remediation) |

No memory leak found.
No `unsafe`, no `Box::leak`, no `Rc`/`Arc` cycle, no global mutable state anywhere in the workspace.
No transaction leak found in the normal paths: `Tx` is finished by `commit`/`rollback`, sqlx rolls back on drop, and `Db::transaction` is a correct scoped boundary apart from finding 8.

The parameterisation discipline is genuinely good: `Renderer::bind` is the only path values take, and the LIKE-escaping fix over QueryDSL (explicit `ESCAPE '!'`, escaping applied unconditionally, backslash deliberately avoided) holds up under review.

---

## 1. HIGH - Generated Rust source injection from database names

`crates/oxider-query-codegen/src/sqlite.rs:56,68` write the raw DB name into a Rust string literal with no escaping:

```rust
writeln!(fields, "    #[oxider(column = \"{column_name}\")]");
write!(out, "#[derive(Entity)]\n#[oxider(table = \"{table}\")]\npub struct {} ...");
```

**Correction to the original write-up.**
The first demo in this report used a table named `t)] pub struct Evil; //`, and claimed the `)]` closed the attribute.
It does not: a payload with no `"` in it stays inside the string literal, Rust tokenises the whole thing as one attribute, and nothing escapes.
The genuine vector needs a `"` to close the literal first, and it was reproduced with a column name:

```
a", skip)] pub x: i64, } pub struct Pwn { pub y: i64 } pub struct Tail { pub z: i64
```

Before the fix, `syn::parse_file` over the generated file returned three top-level items instead of one:
`["struct T", "struct Pwn", "struct Tail"]`.
Nothing downstream flags it, and the injected items are real items in the user's crate.
Codegen is documented for use from a build script, so injected code compiles and runs inside the developer's / CI toolchain.

Threat model: any schema the developer does not fully control - a shared or multi-tenant dev database, a restored dump, an app that creates tables from user input.

Fix: escape `"` and `\`, reject control characters and newlines, or emit the literal via `proc_macro2::Literal` / `format!("{:?}", name)` instead of hand-quoting.
Add a test asserting a hostile name round-trips as data, not as code.

## 2. MEDIUM - Identifier interpolation into `PRAGMA`

`crates/oxider-query-codegen/src/sqlite.rs:39`:

```rust
// `table` comes from sqlite_master (trusted); quote it as an identifier.
let columns = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
```

The comment's premise is wrong: a SQLite table name may contain `"`.
`CREATE TABLE "ev""il" (...)` makes codegen fail with `near "il": syntax error`, i.e. the quoting is escapable.

Fix: `SELECT * FROM pragma_table_info(?)` (the table-valued form does accept a bind parameter), or double the quote via the same `quote_with` helper the core dialects already use.

## 3. MEDIUM - Unescaped identifier splice in `NEXTVAL` / `CURRVAL`

`typed/functions.rs:95` builds `Node::Keyword(sequence)`; `dialect/templates/agg.rs:71` renders `NEXTVAL('<Elem::Ident>')`; `render/expr.rs:155` pushes a `Keyword` verbatim.
This is the only place the crate builds a SQL string literal by concatenation.

```
next_val::<i64>("s'); DROP TABLE users; --")
-> SELECT NEXTVAL('s'); DROP TABLE users; --') FROM "users"
```

Exploitation needs a `&'static str` (see finding 15), so this is defence-in-depth rather than a remotely reachable hole - but the escape is simply absent, and the crate's stated selling point is that nothing reaches the SQL by concatenation.

Fix: double `'` for the quoted PostgreSQL form, and route the ANSI `NEXT VALUE FOR <name>` form through `quote_ident`.

## 4. MEDIUM - Recursive renderer aborts the process on a deep expression

`Renderer::expr_within -> node -> operation -> expr_within` recurses once per AST level.
Building and dropping the AST are iterative and fine; only rendering overflows.

Measured (release, 2 MB test-thread stack): 5,000 chained `filter(...)` calls render fine, 10,000 overflow the stack.
A stack overflow aborts the process (`STATUS_STACK_OVERFLOW`); it is not a catchable panic.

`filter_opt` in a loop over a caller-supplied filter list is the documented dynamic-query pattern, so an unbounded list from an API turns into a whole-process DoS.

Fix: carry a depth counter in `Renderer` and return `RenderError::Invalid("expression nested too deeply")` past a limit (a few hundred is far beyond any real query), or fold repeated `And`/`Or` chains into an n-ary node so depth stays bounded.

## 5. MEDIUM - Set-operation branches emit their own tail clauses

`render/select.rs:186` renders each branch with the full `select()`, which includes the branch's `WITH`, `ORDER BY`, `LIMIT` and locking.
PostgreSQL wraps branches in parentheses so this is valid; SQLite sets `wrap_set_op_branches: false`, so:

```
SELECT "users"."id" FROM "users" UNION SELECT "users"."id" FROM "users" ORDER BY "users"."id" DESC LIMIT 5 LIMIT 3
-> sqlite: near "LIMIT": syntax error   (confirmed against a real DB)

SELECT ... UNION WITH "c" AS (SELECT ...) SELECT ...
-> invalid SQL
```

Even where it parses, an unwrapped branch `ORDER BY` silently rebinds to the whole set operation.

Fix: refuse at render time when an unwrapped branch carries `with`/`order`/`limit`/`offset`/`lock`, or always wrap branches and drop the cap.

## 6. MEDIUM - `UPDATE` with no assignments renders invalid SQL

`render/dml.rs:120` pushes `" SET "` unconditionally.

```
User::update().filter(User::id.eq(1))
-> UPDATE "users" SET  WHERE "users"."id" = $1
-> sqlite: near "WHERE": syntax error   (confirmed against a real DB)
```

`INSERT` already guards the analogous empty case with `RenderError::Invalid`; `UPDATE` and `ON CONFLICT DO UPDATE SET` do not.
Fix: same guard, same error type.

## 7. MEDIUM - `UPDATE ... FROM` and `DELETE ... USING` are not capability-gated

The crate's stated contract is that a dialect refuses at render time what it cannot run.
These two constructs bypass it - `Caps` has no flag for either.

- `Delete::using` on SQLite: `DELETE FROM "users" USING "depts" WHERE ...` -> `near "USING": syntax error` (confirmed against a real DB). SQLite has no multi-table delete at all.
- `Update::from` on MySQL: renders `UPDATE ... SET ... FROM ...`, which MySQL does not support (it spells this `UPDATE t1 JOIN t2 SET ...`). Not run against a live MySQL here.
- `Delete::using` on MySQL: MySQL requires the target table to also appear in the `USING` list, so the emitted form fails there too. Not run against a live MySQL here.

Fix: add `update_from` and `delete_using` to `Caps` (SQLite: `update_from = true` since 3.33, `delete_using = false`; MySQL: both false; Postgres: both true) and check them in `render_update` / `render_delete`.

## 8. MEDIUM - `Db::transaction` loses the caller's error when rollback fails

`crates/oxider-query-exec/src/db.rs`:

```rust
Err(err) => {
    tx.rollback().await?;   // `?` returns the ROLLBACK error and drops `err`
    Err(err)
}
```

If the connection is already broken - the common reason a statement failed in the first place - the rollback also fails and the caller sees the rollback error instead of the real cause.
Fix: `let _ = tx.rollback().await; Err(err)`, logging the rollback failure if a logging facade is added later.

Related, lower priority: `Db::connect` uses `Pool::connect` defaults (no `max_connections`, `acquire_timeout` or `idle_timeout` knobs exposed) and `Db` has no `close()`, so callers cannot drain the pool at shutdown.
`Db::new(pool)` is the current workaround and worth documenting.

## 9. MEDIUM - Silent no-ops that weaken a safety guarantee

- `Select::skip_locked()` / `no_wait()` are no-ops when no `for_update()` was set. `User::query().select(User::id).skip_locked()` renders `SELECT "users"."id" FROM "users"` - no locking at all (confirmed). The documented use case is the queue-worker pattern, where silently losing the lock means several workers process the same rows.
- `Window::exclude()` is a no-op when no frame was set.

Fix: make these type-state transitions (only available on a locked / framed builder), or return a render error, or at minimum have `skip_locked` / `no_wait` imply `FOR UPDATE`.
The same design already got this right elsewhere: `filter_opt` takes an explicit `Option` precisely so a missing filter cannot silently widen a result set.

## 10. LOW-MEDIUM - `Insert` silently drops data when spellings are mixed

Confirmed:

- `insert().from_query(q).set(User::name, "x")` -> `INSERT INTO "users" ("name") SELECT "users"."id" FROM "users"` - column list and projection no longer match; the value is dropped but the column name is kept.
- `insert().set(User::name, "a").columns((User::id,)).values((1,))` -> `INSERT INTO "users" ("id") VALUES ($1)` - the earlier `set` value is discarded silently.
- `insert().from_query(q).values(...)` -> the row is silently dropped.

Fix: type-state (as `Insert<E, C>` already does for `set` vs `columns`) so the remaining combinations do not compile, or a render-time `Invalid`.

## 11. LOW - `param()` cannot be bound

`typed/functions.rs:34` returns `Node::NamedParam`, and `render/expr.rs:42` is the only other reference: it always returns `RenderError::Invalid("a named parameter was not bound before rendering")`.
There is no binding API anywhere in the workspace.
Any query using the public `param()` fails at render, always.
Fix: implement the bind step, or remove `param` / `NamedParam` until it exists.

## 12. LOW - Empty `IN` renders as an unparenthesised keyword

`ops_compare.rs` returns `Node::Keyword("1 = 0")`, which the renderer treats as an atom (`precedence::HIGHEST`), so it is never wrapped.
Inside `AND` / `OR` / `NOT` it happens to parse correctly, but composing it further breaks:

```
User::id.in_values(Vec::<i64>::new()).eq(true)
-> WHERE 1 = 0 = $1
```

Fix: emit `Node::Keyword("(1 = 0)")`, or build a real `Op(Eq, [Param(1), Param(0)])` node so precedence handling applies.

## 13. LOW - `IndexOfFrom` emulation is wrong when the needle is absent

ANSI: `(POSITION(needle IN SUBSTRING(hay FROM start)) + start - 1)`; SQLite: the `INSTR` equivalent.
When the needle is not found the inner call returns `0`, so the expression yields `start - 1` instead of `0`.
No public builder method reaches `IndexOfFrom` today (only `index_of`), so this is latent rather than live.
Fix: wrap in `CASE WHEN inner = 0 THEN 0 ELSE inner + start - 1 END`, and add the method with a test.

## 14. LOW - Parameter values are deep-cloned twice per execution

`Renderer::bind` does `self.params.push(value.clone())`, and the SQLite backend's `bind_params!` does `query.bind(s.clone())` again.
For a large `Value::Bytes` / `Value::Text` that is two full copies per statement.
Not a leak - all of it is freed - but avoidable memory pressure on blob-heavy workloads.
Fix: have the renderer move values out of the consumed AST, and let the backend bind by reference where sqlx allows it.

Related: the MySQL / SQLite `Sinh`, `Cosh`, `Tanh` and `Coth` templates repeat `Arg(0)`, so the argument is rendered - and its parameters bound - twice, and a volatile argument is evaluated twice by the engine.
Correct SQL, but worth a doc note.

## 15. INFO - `&'static str` is the whole identifier-injection defence

Table names, aliases, CTE names, `col()`, `star_of()`, `raw()` and sequence names are all `&'static str`, which is what keeps runtime data out of the SQL text.
Good design, but `&'static str` does not actually mean "compile-time constant": `String::leak` / `Box::leak` are safe stable Rust, so `col(user_input.leak(), "x")` compiles - and leaks memory on every call.

Most such paths go through `quote_ident`, so even a leaked string is quoted and escaped.
The exceptions are exactly finding 3 (`next_val` / `curr_val`) and the documented `raw()` escape hatch.

Recommend a short "Security model" section in `README.md` / `docs/` stating: identifiers must come from literals or codegen, never from `leak()`; `raw()` is a trusted-input escape hatch; values always go through `bind` / `Param`.

## 16. INFO - CI gaps

`.github/workflows` runs `cargo test --all`, not `--all-features`, so `chrono`-gated code is compiled but never exercised in CI (clippy does use `--all-features`).
No `cargo audit` / `cargo deny` step.
The current lockfile is clean on the versions that matter (sqlx 0.8.6 is past RUSTSEC-2024-0363, libsqlite3-sys 0.30.1, chrono 0.4.45), but nothing keeps it that way.

## 17. MEDIUM - `Node`'s derived `Drop` recurses and aborts on the same deep tree (found during remediation)

Not in the original sixteen; found only because the depth guard for finding 4 did not fix the crash.

With `MAX_DEPTH` in place, building a 20 000-deep predicate chain and rendering it correctly returned `RenderError::TooDeep` - and the process still aborted, after the render, when the tree was dropped.
The derived `Drop` walks `Box`/`Vec` children recursively, so freeing a deep tree overflows the stack exactly as rendering it did.
A render guard cannot help: the tree is already built by then.

Fix: an explicit `impl Drop for Node` that detaches children onto a worklist and frees them iteratively.
`Node::Subquery` is deliberately left to the derived recursion, because it holds a `SelectAst` rather than a `Node` and a worklist over it would have to duplicate the whole AST shape; nesting subqueries thousands deep is not a shape a builder API produces by accident, and `MAX_DEPTH` still refuses to render it.

Cost, and it is a real one: a type with `Drop` cannot be destructured by value, so `Node` lost `match node { Node::Op(op, args) => ... }`-style moves.
`Node::connect` had to be rewritten against `&mut`.
`Node` is public, so this is a public API change, not only an internal one.

Related, and the reason the guard alone was not enough in practice: chained `AND`/`OR` used to nest one level per clause, so a dynamic filter loop over a few hundred inputs built a deep tree in the ordinary case.
`Node::connect` now appends into an existing `And`/`Or` node instead, making those chains flat, so `MAX_DEPTH` constrains only genuinely nested shapes.

---

## Remediation

Every finding above was fixed in the same session, under
`plans/260908-0957-security-and-query-audit-remediation/plan.md`.

| # | Fix | Regression test |
|---|-----|-----------------|
| 1 | `escape_rust_string` on every name written into generated source | `codegen/tests/sqlite_introspection.rs` asserts a hostile name yields exactly one item |
| 2 | `SELECT * FROM pragma_table_info(?)` with a bound name | same file: a table name containing a quote introspects |
| 3 | `Elem::TextLiteral` quotes the sequence name and doubles inner quotes | `expression_scenarios.rs` |
| 4 | `MAX_DEPTH` (256) + `RenderError::TooDeep`, flattened `AND`/`OR` | `dynamic_query_depth_scenarios.rs` |
| 5 | branch-tail check, refusing where the dialect does not wrap branches | `set_operation_and_cte_scenarios.rs` |
| 6 | `RenderError::Invalid` on an assignment-free `UPDATE` / `SET` | `dml_scenarios.rs` |
| 7 | `Caps::update_from` / `Caps::delete_using` | `dml_scenarios.rs`; both refusals confirmed against MySQL 8.4 |
| 8 | rollback failure ignored, caller's error returned | `sqlite_end_to_end.rs` |
| 9 | `Unlocked`/`Locked` and `NoFrame`/`Framed` type-states | `compile_fail` doc tests |
| 10 | `NoRows`/`OneRow`/`FromQuery` plus the `AcceptsQuery` marker | `compile_fail` doc tests |
| 11 | `Bindings` on all four builders, `bind(name, value)`, `RenderError::UnboundParameter` | `named_parameter_scenarios.rs` |
| 12 | empty `IN` renders `(1 = 0)` / `(1 = 1)` | `select_scenarios.rs` |
| 13 | `CASE WHEN <inner> = 0 THEN 0 ELSE <inner> + start - 1 END` | `sqlite_end_to_end.rs`, checked against MySQL's native `LOCATE` |
| 14 | exec binds `&str`/`&[u8]` instead of cloning | - |
| 15 | security model documented | `docs/16-security-model.md`, README |
| 16 | CI gains `--all-features`, a separate doc-test run, and `cargo audit` | - |
| 17 | iterative `impl Drop for Node` | `dynamic_query_depth_scenarios.rs` |

### Corrections to this report

- **Finding 1's original evidence was wrong**, as noted in that section. The finding itself stands; the payload shown did not demonstrate it.
- **One concern was dropped as incorrect.** The report's unresolved question 1 suspected that `SELECT DISTINCT` plus the emulated `NULLS FIRST`/`NULLS LAST` sort key would be rejected by MySQL under `ONLY_FULL_GROUP_BY`. It runs fine on MySQL 8.4. No fix was made and none is needed.
- **Finding 7's MySQL half is now confirmed, not inferred.** On MySQL 8.4: `UPDATE posts SET ... FROM users ...` is ERROR 1064, and `DELETE FROM posts USING users ...` is ERROR 1109 `Unknown table 'posts' in MULTI DELETE`, because MySQL's `USING` lists every table including the target rather than only the extra ones. Refusing at render time is right.
- **Finding 14 is half-fixed.** The exec-side clone is gone; the renderer still clones each `Value` into the parameter list, because removing it needs `render_with` to consume the AST rather than borrow it. Deferred deliberately.

### Answers to the unresolved questions

1. A MySQL 8.4 container was used; the results are folded into findings 7 and 13 above, and one suspicion was withdrawn.
2. Codegen escaping was fixed regardless of the trust assumption, and the trust boundary is now written down in `docs/16-security-model.md`.
3. `skip_locked` / `no_wait` without a lock is now a compile error via the `Unlocked`/`Locked` type-state.
4. `param()` ships; `bind` on each builder is its counterpart, and an unbound name is a render error.

### Left open

- The renderer-side `Value` clone (finding 14, first half).
- `Node::Subquery` still drops recursively, so a tree thousands of subqueries deep can still overflow on drop. Rendering it is refused either way.
- `cargo audit` is wired into CI but was never run locally; no network in this session to install it.

---

## Suggested order of work

1. Findings 1 and 2 - codegen escaping. Smallest diff, highest severity.
2. Findings 6, 7, 5 - render-time refusals. All three restate the principle the crate already claims ("refuse here rather than emit plausible-but-wrong SQL"); each is a few lines plus a test.
3. Finding 4 - depth guard in the renderer.
4. Findings 3, 8, 9, 11.
5. Findings 10, 12, 13, 14, 15, 16.

Each is independent and belongs in its own commit.

## Unresolved questions

1. Finding 7 (MySQL half) and the `SELECT DISTINCT` + emulated `NULLS LAST` case were reasoned from dialect docs, not run against a live MySQL. SQLite accepts the `DISTINCT` case; MySQL under `ONLY_FULL_GROUP_BY` is expected to reject the emulated sort key. Is a MySQL instance available to confirm, or should MySQL stay best-effort until the `oxider-query-exec` MySQL backend lands?
2. Is `oxider-query-codegen` intended to run against untrusted schemas (shared dev DB, customer dump), or is a trusted-schema assumption acceptable and to be documented instead of fixed? The fix is cheap either way, so my recommendation is to fix it regardless.
3. Should `skip_locked` without `for_update` be a compile error (type-state), a render error, or implicitly add `FOR UPDATE`? This changes the public API shape, so it is a product call.
4. Is `param()` meant to ship in 0.1.0? If named parameters are planned, keeping the API is defensible; if not, removing it before the API stabilises is cheaper.

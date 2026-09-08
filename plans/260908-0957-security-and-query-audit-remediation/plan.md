# Audit remediation - security, transaction, memory, query correctness

Status: done
Source: `plans/reports/security-and-data-query-audit-260908-0944-oxider-query-render-exec-codegen-report.md`

## Decisions taken

- MySQL claims verified E2E against MySQL 8.4 in Docker (`oxider-audit-mysql`, port 33061).
  `UPDATE ... FROM` -> ERROR 1064. `DELETE FROM t USING u` -> ERROR 1109 MULTI DELETE. Both confirmed.
  `SELECT DISTINCT` + emulated null-ordering sort key runs fine on MySQL 8.4 -> that concern was wrong, dropped.
- Codegen escaping: fix regardless of the trusted-schema assumption.
- `skip_locked` / `no_wait` without a lock: compile error (type-state).
- `param()` ships: implement named-parameter binding.

## Phases

| Phase | Scope | Findings | Status |
|-------|-------|----------|--------|
| 01 | codegen: escape generated Rust literals, parameterise PRAGMA | 1, 2 | done |
| 02 | render-time refusals: empty UPDATE SET, set-op branch tails, UPDATE FROM / DELETE USING caps | 5, 6, 7 | done |
| 03 | renderer depth guard | 4 | done |
| 04 | expression fixes: sequence-name escaping, empty IN parens, IndexOfFrom | 3, 12, 13 | done |
| 05 | exec: preserve caller error on rollback failure, bind by reference | 8, 14 | done |
| 06 | type-state: locking wait policy, insert source mode | 9, 10 | done |
| 07 | named parameter binding | 11 | done |
| 08 | docs security model + CI hardening | 15, 16 | done |

## Acceptance criteria

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` all clean.
- Every fixed finding has a regression test; the SQL-level ones are proved against a real engine (SQLite in-process, MySQL in Docker) rather than only against a rendered string.
- Misuse that used to render silently wrong SQL either fails to compile or returns `RenderError`.
- No public API left that can only fail (`param`).

## Verification run

All four gates clean on 2026-09-08 after phase 08:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets --all-features` (19 suites, 0 failures)
- `cargo test --workspace --all-features --doc` (18 doc tests, including 5 `compile_fail`)

`cargo audit` could not be run locally (no network to install it); it is wired into CI instead.

## Notes

- Renderer-side `Value` clone (finding 14 first half) needs `render_with` to consume the AST; deferred, documented in the final report.
- Duplicated `Arg(0)` in the hyperbolic templates is correct SQL with double evaluation; documented, not changed.

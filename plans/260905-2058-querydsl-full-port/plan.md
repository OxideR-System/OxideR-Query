# OxideR-Query: port toàn bộ QueryDSL sang Rust + trang docs Docusaurus

Status: KHÉP LẠI. Mục tiêu đã đổi sang "query builder tốt nhất cho Rust"; phần việc còn lại chuyển sang `plans/260909-1018-rust-first-roadmap/plan.md`.
Phase 1-6, 7 và 9 xong. Phase 7 khép lại 2026-09-09: Postgres từ commit `fa4b8ef` (v0.2.0), MySQL sau đó.
Ngày tạo: 2026-09-05
Cập nhật: 2026-09-09
Baseline khi bắt đầu: v0.1.0, 38 test xanh.
Hiện tại: 166 test + 13 doc test xanh, clippy sạch, fmt sạch, trang Docusaurus build được.

## Mục tiêu

Học cách QueryDSL (Java) hiện thực từng tính năng, viết lại đầy đủ cho Rust trong workspace này, kèm một trang tài liệu Docusaurus hướng dẫn chi tiết cho người dùng.

Định vị giữ nguyên: query builder type-safe, layered, đa dialect, sinh `(sql, params)`; execution là lớp optional.
Không làm ORM, không tự viết driver.

## Nguồn tham chiếu

QueryDSL source: `../querydsl` (fork OpenFeign).
Báo cáo nghiên cứu: `reports/from-researcher-to-planner-querydsl-*.md` (6 báo cáo: ops + expression hierarchy, templates + dialects, SQL query surface, projections + grouping, dynamic paths + metadata, codegen + multi-backend).

## Quyết định kiến trúc đã chốt

### 1. Operator + Template, thay cho match cứng trong renderer

QueryDSL tách `Operator` (enum phẳng) khỏi `Templates` (bảng `Operator -> template string`), mỗi dialect override vài entry.
Renderer chỉ tra bảng rồi điền tham số.
Port sang Rust: `Operator` enum + `Template(&'static [Elem])` + `Dialect::template(op)` có default impl.

Đã làm. Bảng ANSI ở `dialect/templates/`, mỗi dialect override phần khác biệt, cộng một hàm `lacks(op)` cho những toán tử engine không có.

### 2. Một kiểu biểu thức typed duy nhất: `Expr<S, T>`

`S` là tập entity được tham chiếu (type-level), `T` là kiểu Rust của kết quả.
Toán tử là method của trait, gate bằng trait bound trên `T`.

Đã làm. `Predicate<S>` là alias của `Expr<S, bool>`; `Column<E,T>` lift lên qua `IntoExpr`.

### 3. `IntoExpr<T>`: gộp "so với giá trị" và "so với cột" thành một method

Đã làm. Bộ impl cuối cùng:

```rust
impl<E, T> IntoExpr<T> for Column<E, T>
impl<S, T> IntoExpr<T> for Expr<S, T>
impl<T: ToSqlValue> IntoExpr<T> for T
impl<T: ToSqlValue> IntoExpr<T> for Option<T>
// cộng danh sách widening tường minh
```

`ToSqlValue` cố tình KHÔNG implement cho `&str`: làm vậy sẽ tạo nhập nhằng với impl widening.
`Raw::bind` nhận `impl Into<Value>` chứ không phải `ToSqlValue` vì lý do đó.

### 4. Alias bảng ở TYPE LEVEL, không phải chỉ runtime

Quyết định này đã đổi so với bản kế hoạch đầu.

Kế hoạch ban đầu định để alias chỉ ở runtime, mọi alias của cùng entity dùng chung marker `E`.
Thử thì self-join không biên dịch được: `E0283`, trình biên dịch tìm được hai chứng cứ `Contains<User>` và không chọn được cái nào.

Giải pháp: `Aliased<E>` là một entity riêng ở tầng type.
`User::id.at("m")` trả `Column<Aliased<User>, i64>`, `User::table().alias("m")` trả `Table<Aliased<User>>`.
Hai bản sao thành hai thành viên khác nhau trong tập phạm vi nên mỗi tham chiếu có đúng một chứng cứ.

Giới hạn còn lại, đã ghi trong docs: mỗi entity chỉ một alias trong một query; bản sao thứ ba phải dùng `col` và mất kiểm tra phạm vi.

### 5. Nullability KHÔNG vào type

Đã thử `IntoExpr<Option<T>>` để `Option<T>` nới rộng ở mọi vị trí.
Kết quả: `E0283`/`E0284` ở khắp nơi, mọi vị trí kiểu tự do thành nhập nhằng (`case_when(cond, "minor")`, `coalesce(User::email)`, `least(...)`, `Post::views.rem(2)`).
Đã revert.

Nullability là cờ runtime trên `Column`. Ghi rõ đánh đổi trong `docs/02` và `docs/14`.

### 6. Chiến lược test: test case theo kịch bản, E2E cho case khó

Theo chỉ đạo của user.

Đã làm. Test case ở `crates/oxider-query/tests/`, mỗi file một nhóm tính năng, assert đầy đủ chuỗi SQL và danh sách params cho mọi dialect diễn đạt được.
E2E ở `crates/oxider-query-exec/tests/sqlite_end_to_end.rs`, chạy SQL thật và assert kết quả trả về.

Cách làm này bắt được bug thật, không phải hình thức:

- SQLite render `STDDEV(...)` qua fallback ANSI dù không có hàm đó.
- `XOR` và `IS NULL` sinh `active <> email IS NULL`, parse sai. Phải thêm mức `precedence::IS` và cho MySQL override `Xor => XOR`.
- CTE đệ quy dựng bằng `join_query` bị SQLite từ chối `circular reference`. Phải thêm `join_name`/`join_name_as`.
- `#[derive(Entity)]` sinh tên cột `"r#type"` cho field raw identifier.
- Codegen sinh mã không parse được với cột tên `type`, `total-count`, `2fa enabled`.
- `Column::in_schema` truyền nhầm tên cột làm tên bảng.

## Trạng thái từng phase

| Phase | Nội dung | Trạng thái |
|---|---|---|
| 1 | Nền tảng: `Operator` + template engine + `Expr<S,T>` + `IntoExpr` | XONG |
| 2 | Bộ toán tử đầy đủ: so sánh, null, in, between, string, math, datetime, case, coalesce, cast, quantifier | XONG |
| 3 | Bề mặt SELECT: mọi loại join, alias và self-join, distinct(-on), nulls ordering, locking, set op, CTE, window function, subquery tương quan | XONG |
| 4 | DML đầy đủ: insert nhiều dòng, insert-select, upsert, RETURNING, update-from, delete-using | XONG (MERGE bỏ: chỉ Oracle/SQL Server, ngoài ba dialect đang hỗ trợ) |
| 5 | Projection và mapping: tuple projection, `#[derive(Projection)]`, GroupBy transformer | MỘT PHẦN: tuple projection arity 2..=12 xong; derive và transformer chưa |
| 6 | Query động và escape hatch: `filter_opt`, `and_opt`/`or_opt`, raw SQL có tham số, named param | XONG |
| 7 | Exec: backend Postgres và MySQL | XONG |
| 8 | Codegen: introspect Postgres và MySQL, PK/FK/index | CHƯA |
| 9 | Trang docs Docusaurus, tiếng Việt | XONG |
| 10 | Chất lượng: ma trận test, bench, CI, README và docs đồng bộ | MỘT PHẦN: test/bench/README/docs xong; CI chưa |

## Tài liệu

`docs/` là bản duy nhất của tài liệu: đọc được dạng Markdown trên GitHub, và được `website/` render thành trang Docusaurus qua `path: '../docs'`.
Không có bản sao thứ hai để lệch nhau.

15 chương, tiếng Việt, `sidebar_position` trong front matter quyết định thứ tự.

Mọi đoạn SQL in trong docs đều được ghim bằng test: hoặc chép từ một scenario có sẵn, hoặc assert trong `crates/oxider-query/tests/documented_example_scenarios.rs`.

## Acceptance criteria toàn cục

| Tiêu chí | Trạng thái |
|---|---|
| `cargo test --workspace --all-targets` xanh | 166 test |
| `cargo test --workspace --doc` xanh | 13 test, 9 trong đó là `compile_fail` |
| `cargo clippy --workspace --all-targets` sạch | sạch |
| `cargo fmt --all --check` pass | pass |
| Mọi tính năng SQL mới có test case assert SQL và params đầy đủ | đạt |
| Case khó có E2E chạy DB thật | 16 test trên SQLite |
| Trang Docusaurus build được | `npm run build` sạch, không warning |
| File nguồn Rust dưới ~200 dòng khi tách được | phần lớn đạt; `builder/select.rs` vượt vì là một kiểu duy nhất |

## Lệnh build trên máy này

`cargo.exe` và `rustc.exe` trong `~/.cargo/bin` là symlink tới `rustup.exe`, Windows từ chối traverse với os error 448.
Phải gọi thẳng toolchain, và Bash tool phải chạy với `dangerouslyDisableSandbox: true`:

```bash
export TC=/c/Users/Admin/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin
export RUSTC="$TC/rustc.exe" RUSTDOC="$TC/rustdoc.exe"
export PATH="$TC:$PATH"
"$TC/cargo.exe" test --workspace --all-targets
"$TC/cargo-clippy.exe" --workspace --all-targets
```

## Việc còn lại

Chuyển hết sang `plans/260909-1018-rust-first-roadmap/plan.md`, nơi mục tiêu đã đổi và thứ tự ưu tiên được xếp lại.
Tóm tắt cái còn nợ khi khép plan này: Phase 5 (`#[derive(Projection)]`, GroupBy transformer), Phase 8 (codegen Postgres/MySQL) và Phase 10 (workflow CI).

E2E qua Docker cho cả Postgres lẫn MySQL đã có, bật bằng `OXIDER_POSTGRES_URL` / `OXIDER_MYSQL_URL`, thiếu env thì skip.

## Câu hỏi chưa giải quyết

1. Docs hiện chỉ có tiếng Việt. Có cần bản tiếng Anh song song qua i18n của Docusaurus không.
2. Kiểu ngày giờ: hiện dùng `chrono` sau feature flag mặc định bật. Có cần hỗ trợ `time` song song không.
3. Trang docs deploy ở đâu. Config đang đặt sẵn cho GitHub Pages tại `oxider-system.github.io/OxideR-Query/`.

# OxideR-Query: Kiến trúc & Roadmap

Status: DONE - Phase 0-7 hoàn tất (exec/codegen: SQLite); 3c hoãn sang Phase 6 (chưa có consumer)
Ngày: 2026-08-14 (cập nhật 2026-08-15)

## Quyết định 3c (2026-08-15): hoãn nullability type-level sang Phase 6

Nullability outer join ở tầng type chỉ quan sát được khi có tầng mapping row->struct/tuple (cột LEFT JOIN phải hiện ra là `Option<_>`).
Với API assoc-const hiện tại (`Department::name` là global const, không mang ngữ cảnh query), ép nullability vào type sẽ tạo type machinery không có consumer -> vi phạm YAGNI + nguyên tắc "type-state chỉ ở nơi trả về giá trị thật".
Quyết định: giữ `nullable` ở runtime trong `Column` + join kind trong `SelectQuery`; tính nullability hiệu dụng khi xây tầng mapping (Phase 6).

## Quyết định exec: handle `Db` tự giữ dialect (2026-08-15)

Thay vì hàm tự do nhận `&Rendered` (buộc call site gọi `.render(&Sqlite)`), lớp exec expose một handle `Db<DB>` generic trên backend.
Core thêm trait `Renderable` (impl cho `Select`/`SelectQuery`/`Insert`/`Update`/`Delete`/`Rendered`); `Db` nhận thẳng query, tự render theo dialect của backend.
`Backend` trait ánh xạ sqlx `Database` -> dialect (assoc `Dialect: Default`) + bind param + rows_affected; encode concern nằm trong impl từng backend nên `Db` backend-agnostic.
Hôm nay chỉ impl `Backend for sqlx::Sqlite` (alias `SqliteDb`); thêm Postgres/MySQL = thêm một impl, không đụng `Db`.
Lợi ích: đổi DB = đổi kiểu handle, không sửa dòng query nào; không rải `.render(&dialect)` khắp code; một API duy nhất thay vì mỗi backend một bộ hàm (tránh name clash ở crate root).

## Pivot thiết kế Column (2026-08-14, sau brainstorm)

Đổi `Column<Sql>` (SQL marker) -> `Column<Entity, T>` (entity + kiểu Rust thật) + associated-const API.
Lý do: entity param cần cho join type-safe (`eq_column`) và projection type-match; kiểu Rust thật cần cho custom domain type + projection.
API mới: `User::id` (assoc const), `User::query()` (Entity trait), thay cho `User::table().id` / `Query::select()`.
Bổ sung: ops gate theo trait (Orderable cho lt/gt, String cho contains/like), ORDER BY / LIMIT / OFFSET, filter_opt (dynamic query).
Đã verify: sai kiểu / sai op fail compile với message đọc được (`no method named contains`, `bool: Orderable not satisfied`, `i64: Into<String>`).

## Phase 3b: type-level table-set enforcement (2026-08-15)

`Select<S>` mang type-level cons-list các entity trong scope (FROM + mỗi JOIN).
`source.rs`: HList (`Cons`/`Nil`), `Contains<E, Idx>` dùng witness `Here`/`There` (frunk trick, tránh overlap), `ContainsAll<List, Idxs>`, `Concat`.
Column ops trả `Predicate<Cons<E, Nil>>` / `Order<Cons<E, Nil>>`; `and`/`or` merge source qua `Concat`; `eq_column` gộp 2 entity.
`filter`/`select`/`order_by` bound `S: ContainsAll<referenced, Idxs>` -> ref cột entity chưa join fail compile.
`join`/`left_join` trả `Select<Cons<E2, S>>`; ON clause KHÔNG check (entity mới chưa trong scope), erase qua trait `OnClause` để turbofish chỉ cần `E2`.
Verify: `User::query().filter(Department::name.eq(...))` khi chưa join Department -> `Nil: Contains<Department>` not satisfied; đã khoá bằng compile_fail doctest.

## Quyết định bổ sung (đã chốt)

- MSRV: 0.x bám latest stable (không cam kết backward-compat); từ 1.x mới yêu cầu tương thích ngược.
- Async runtime (Phase 6): tokio.
- Exec layer: tận dụng sqlx.

## Trạng thái MVP

Workspace 3 crate build xanh, `cargo test/clippy -D warnings/fmt` sạch.
Đã có: `#[derive(Entity)]` sinh metamodel, SELECT + typed WHERE (eq/ne/lt/le/gt/ge, and/or), render Postgres ra `(sql, params)`.
Đã verify type-safety: so sánh cột khác kiểu SQL fail compile với message đọc được (`i64: IntoExpr<Text> is not satisfied`).
Chưa làm (đúng scope MVP): type-state ordering, column-belongs-to-table enforcement, nullability ở tầng type, JOIN, dialect khác Postgres.

## Mục tiêu

Thư viện dựng query type-safe cho Rust, lấy cảm hứng từ QueryDSL (Java) nhưng đẩy type-safety xa hơn nhờ type system của Rust.
Định vị: **query builder layered** (DB-agnostic), sinh ra `(sql_string, params)`, không tự quản connection ở core.

## Quyết định đã chốt (từ user)

- Phạm vi: query builder layered, execution/mapping là lớp optional lên trên.
- Metamodel: hỗ trợ CẢ derive macro (entity bằng Rust struct) LẪN introspect schema DB. Triển khai derive macro trước.
- Mục tiêu: học rồi nâng lên production. Thiết kế chuẩn prod ngay từ đầu (CI, test, docs), nhưng scope theo phase.
- Backend: đa dialect SQL (Postgres, MySQL, SQLite). Postgres chạy vững trước rồi nhân rộng qua trait `Dialect`.

## Nguyên tắc thiết kế cốt lõi

1. **DX là killer feature, không chỉ type-safety.**
   Diesel đã rất type-safe nhưng error message địa ngục + compile chậm khiến người dùng bỏ chạy sang SeaQuery.
   OxideR phải ưu tiên: error đọc được, compile time chấp nhận được, API học nhanh.
2. **Tách AST khỏi rendering.**
   Query build thành một IR/AST dialect-agnostic; `render(&dialect)` mới sinh SQL cụ thể.
   Đây là chìa khoá cho đa dialect mà không nhân bản logic.
3. **Type-state chỉ ở nơi trả về giá trị thật.**
   Dùng type-state builder để ép bất biến quan trọng (thứ tự clause, cột thuộc bảng đã join, nullability).
   KHÔNG type-state hoá mọi thứ - nơi nào type gây error khó đọc mà lợi ích nhỏ thì dùng runtime check.
4. **Feature-gate mọi backend/optional layer** để giữ compile nhẹ.

## Cấu trúc workspace (dự kiến)

```
oxider-query/                  # workspace root
├─ crates/
│  ├─ oxider-query-core/       # AST/IR, trait Expression/Column/Table, Dialect trait, render. Không macro, không DB.
│  ├─ oxider-query-macros/     # proc-macro: #[derive(Entity)] sinh metamodel (Q-types / column consts).
│  ├─ oxider-query-codegen/    # introspect DB schema -> sinh entity (metamodel path #2). Phase sau.
│  ├─ oxider-query-dialects/   # impl Dialect cho Postgres/MySQL/SQLite (hoặc feature-gate trong core).
│  ├─ oxider-query-exec/       # OPTIONAL: execution + row mapping, cầu nối sqlx/tokio-postgres. Phase sau.
│  └─ oxider-query/            # facade crate: re-export + feature flags. Đây là crate user dùng.
└─ examples/, benches/, tests/
```

## Thiết kế type-safety (tóm tắt)

- `trait Table` - metadata bảng (tên, schema).
- `struct Column<Tab, SqlTy, Nullable>` - cột gắn bảng + kiểu SQL + cờ nullable ở tầng type.
- `trait Expression<SqlTy>` - mọi biểu thức (column, literal, binary op) có kiểu SQL đã biết.
  So sánh `.eq()` chỉ nhận Expression cùng SqlTy -> cấm so sánh khác kiểu tại compile-time.
- `SelectBuilder<Source, State>` - `Source` track tập bảng trong FROM/JOIN; chỉ cho tham chiếu cột thuộc `Source`.
  `State` (type-state) ép thứ tự: không gọi `.where_()` khi chưa có FROM.
- Nullability: OUTER JOIN nâng cột non-null của bảng bên phải thành nullable ở tầng type.
- `trait Dialect` - quote ident, placeholder ($1 vs ? vs @p1), LIMIT/OFFSET, bool literal, upsert syntax.

## Roadmap theo phase

| Phase | Nội dung | Deliverable |
|-------|----------|-------------|
| 0 | [DONE] Scaffold workspace, CI (fmt/clippy/test), khung AST core | Workspace build xanh, CI yml có |
| 1 | [DONE] Expression system + Column model + derive macro tối thiểu. SELECT ... WHERE 1 bảng. Render Postgres. | 3 test render xanh, type-safety verified |
| 2 | [DONE] Trait `Dialect` tách module + Postgres/MySQL/SQLite. Khác biệt placeholder ($N vs ?) + quote (" vs `) + escaping | 3 dialect render đúng, unit test escaping |
| 3a | [DONE] JOIN render (INNER/LEFT) + multi-entity select + eq_column join-key type-safe | 3 join test, param order đúng, mismatch key fail compile |
| 3b | [DONE] Type-track bảng đã join (chỉ cho ref cột đã join) qua HList source-set | ref cột chưa join fail compile, compile_fail doctest |
| 3c | (HOÃN sang Phase 6) Nullability outer join ở tầng type - chỉ có ý nghĩa khi có tầng mapping row->struct | test biên nullability |
| 4a | [DONE] Aggregate (COUNT/SUM/AVG/MIN/MAX) + GROUP BY + HAVING, gate Numeric/Orderable | 4 render test, sum(text) fail compile |
| 4b | [DONE] INSERT / UPDATE / DELETE builder + render, single-table type-safe | 5 render test, set cột sai entity fail compile |
| 4c | [DONE] Subquery: IN/NOT IN (type-match cột outer) + EXISTS/NOT EXISTS, uncorrelated | 4 render test, IN sai kiểu fail compile |
| 5 | [DONE - SQLite] Codegen introspect DB schema `oxider-query-codegen` (sqlite_master + PRAGMA table_info -> struct, PK/nullable) | 1 test E2E introspect in-memory SQLite ra source đúng |
| 6 | [DONE - SQLite] Lớp exec `oxider-query-exec` + mapping row->struct (sqlx FromRow), async tokio. Postgres/MySQL sau theo feature | 3 test E2E in-memory SQLite (insert/select/filter/update/delete) |
| 7 | [DONE] DX polish: docs + render-throughput bench (criterion, không so sánh cross-lib) | `cargo doc -D warnings` sạch, bench `render` (simple_select ~1.8µs, join_group_having ~4.7µs) |

Phase 0-4 là "học + xây core vững". Phase 5-7 là "nâng lên production".

## Acceptance criteria (toàn cục)

- Mọi crate: `cargo clippy -- -D warnings` sạch, `cargo test` xanh, `cargo fmt --check` pass.
- Mỗi feature SQL có test render snapshot cho cả 3 dialect (từ Phase 2).
- Type-safety có "trybuild" test: code sai kiểu PHẢI fail compile với message đọc được.
- MSRV cố định, ghi trong CI.

## Rủi ro & lưu ý

- Type-state + generic nhiều tầng dễ tạo error message tệ và compile chậm (bài học Diesel). Đo compile time từ Phase 1, dừng đẩy type-safety nếu DX tụt.
- Đa dialect từ sớm: rủi ro trừu tượng hoá `Dialect` sai. Giảm thiểu bằng cách làm Postgres vững ở Phase 1 rồi mới trích trait ở Phase 2 (trích từ code thật, không thiết kế trên giấy).
- 2 path metamodel: giữ chung một `Entity` representation để derive macro và codegen cùng sinh ra một dạng, tránh 2 hệ thống song song.

## Câu hỏi chưa giải quyết

1. Tên crate publish trên crates.io: `oxider-query` còn trống chưa? (cần check trước Phase 0)
2. MSRV mong muốn: bám latest stable hay giữ tương thích ngược N version?
3. Async runtime cho Phase 6: chỉ tokio, hay runtime-agnostic?
4. Có muốn tương thích/tận dụng `sqlx` ở lớp exec, hay tự viết driver layer?

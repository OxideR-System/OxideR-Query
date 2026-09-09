---
id: index
title: Giới thiệu
slug: /
sidebar_position: 0
---

# Tài liệu OxideR-Query

OxideR-Query là thư viện dựng câu lệnh SQL type-safe, đa dialect cho Rust, lấy cảm hứng từ QueryDSL (Java).

Bạn định nghĩa entity bằng struct Rust thường, derive `Entity`, rồi dựng query.
Trình biên dịch kiểm tra hai điều mà QueryDSL trên JVM không làm được: kiểu của từng cột phải khớp với thứ bạn so sánh, và bảng bạn tham chiếu phải thực sự có trong query.

Query được dựng một lần thành AST độc lập dialect.
Gọi `to_sql(&dialect)` sinh ra cặp `(sql, params)` cho PostgreSQL, MySQL hoặc SQLite.

Bộ tài liệu này đi từ cài đặt tới lúc chạy query trên database thật, và mô tả đầy đủ bề mặt API đã port.

## Mục lục

| Chương | Nội dung |
|---|---|
| [1. Bắt đầu nhanh](./01-getting-started.md) | Cài đặt, entity đầu tiên, query đầu tiên, chạy thử end-to-end. |
| [2. Entity và cột](./02-entities-and-columns.md) | `#[derive(Entity)]`, ánh xạ kiểu, cột nullable, schema, alias bảng. |
| [3. SELECT và lọc dữ liệu](./03-select-and-filtering.md) | Projection, `filter`, sắp xếp, phân trang, `DISTINCT`, khóa dòng, query động. |
| [4. Bộ toán tử](./04-operators.md) | So sánh, null, chuỗi, số học, ngày giờ, `CASE`, `COALESCE`, `CAST`, raw SQL. |
| [5. JOIN](./05-joins.md) | Mọi loại join, self-join qua alias, derived table, kiểm tra phạm vi bảng. |
| [6. Aggregate và GROUP BY](./06-aggregates-and-grouping.md) | `COUNT`/`SUM`/`AVG`/`MIN`/`MAX`, `DISTINCT`, `FILTER`, `GROUP BY`/`HAVING`. |
| [7. Window function](./07-window-functions.md) | `OVER`, `PARTITION BY`, frame, window đặt tên, các hàm xếp hạng. |
| [8. Subquery](./08-subqueries.md) | `IN`, `EXISTS`, subquery vô hướng, `ANY`/`ALL`, subquery tương quan. |
| [9. Set operation và CTE](./09-set-operations-and-ctes.md) | `UNION`/`INTERSECT`/`EXCEPT`, `WITH`, CTE đệ quy. |
| [10. INSERT, UPDATE, DELETE](./10-dml.md) | Insert nhiều dòng, insert-select, upsert, `RETURNING`, update-from, delete-using. |
| [11. Dialect](./11-dialects.md) | Trait `Dialect`, bảng `Caps`, template, độ ưu tiên, những gì mỗi engine từ chối. |
| [12. Thực thi query](./12-execution.md) | Crate `oxider-query-exec`, `Db`, transaction, xử lý lỗi. |
| [13. Sinh entity từ schema](./13-codegen.md) | Crate `oxider-query-codegen`, introspect database có sẵn. |
| [14. Bảo đảm type-safety](./14-type-safety.md) | Những lỗi bị bắt lúc biên dịch, và giới hạn của cách kiểm tra này. |
| [15. Tra cứu nhanh API](./15-api-cheatsheet.md) | Bảng tổng hợp mọi phương thức và toán tử. |
| [16. Mô hình bảo mật](./16-security-model.md) | Ranh giới tin cậy, giá trị và định danh, `raw`, tham số đặt tên, giới hạn độ sâu. |

## Cài đặt

Thư viện chưa publish lên crates.io, nên khai báo phụ thuộc qua Git.
Người dùng chỉ cần crate facade `oxider-query`; nó re-export core và derive macro.

```toml
[dependencies]
oxider-query = { git = "https://github.com/OxideR-System/OxideR-Query" }

# Tùy chọn: lớp thực thi async qua sqlx (PostgreSQL và SQLite)
oxider-query-exec = { git = "https://github.com/OxideR-System/OxideR-Query" }

# Tùy chọn: sinh entity từ schema database có sẵn
oxider-query-codegen = { git = "https://github.com/OxideR-System/OxideR-Query" }
```

## Ví dụ 30 giây

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
    .to_sql(&Postgres)
    .unwrap();

// rendered.sql:
//   SELECT "users"."id", "users"."name" FROM "users"
//   WHERE "users"."name" LIKE $1 ESCAPE '!' AND "users"."age" >= $2
//   ORDER BY "users"."id" DESC LIMIT 20
// rendered.params: ["%nguyen%", 18]
```

Hai chi tiết đáng chú ý ngay từ ví dụ này.

`LIKE` sinh ra kèm `ESCAPE '!'` và giá trị được escape trước khi bọc wildcard.
Tìm chuỗi `50%` sẽ không khớp `500 units`, khác với QueryDSL vốn chỉ escape toán hạng hằng.

`to_sql` trả về `Result`.
Một dialect không diễn đạt được cấu trúc bạn dựng sẽ từ chối ngay tại đây, thay vì sinh SQL sai rồi để database báo lỗi khó hiểu.

## Các crate trong workspace

| Crate | Vai trò |
|-------|---------|
| `oxider-query` | Crate facade người dùng phụ thuộc vào. Re-export core và derive macro. |
| `oxider-query-core` | AST, tầng biểu thức type-safe, builder, trait `Dialect`, renderer. Không DB, không macro. |
| `oxider-query-macros` | `#[derive(Entity)]` sinh metamodel. |
| `oxider-query-exec` | Tùy chọn: thực thi async qua sqlx (PostgreSQL và SQLite). Bind param, map row. |
| `oxider-query-codegen` | Tùy chọn: introspect schema, sinh struct `Entity` từ database có sẵn (SQLite hôm nay). |

## Kiến trúc bốn tầng

Mỗi tầng chỉ biết tầng dưới nó, nên thêm một toán tử hay một dialect không lan ra toàn bộ codebase.

| Tầng | Nội dung | Không biết gì về |
|---|---|---|
| `ast` | Cây không kiểu (`Node`, `SelectAst`, ...) và enum `Operator`. | Dialect, kiểu Rust. |
| `dialect` | Bảng `Operator -> Template` cho từng engine, `Caps`, độ ưu tiên. | Cách đi cây. |
| `render` | Một bộ đi cây duy nhất, nhận `&dyn Dialect`. | Từng toán tử cụ thể. |
| `typed` và `builder` | `Column<E,T>`, `Expr<S,T>`, `Select<S,F>`, kiểm tra lúc biên dịch. | Chuỗi SQL. |

Thêm một toán tử là thêm một dòng vào bảng template, không sửa renderer.
Thêm một dialect là override vài entry, không nhân bản logic.

## Trạng thái

Phiên bản 0.1.0, đang phát triển tích cực.

Đã có: toàn bộ bề mặt SELECT (projection, mọi loại join, alias, `DISTINCT ON`, thứ tự NULL, khóa dòng, set operation, CTE kể cả đệ quy, window function, subquery tương quan), DML đầy đủ (insert nhiều dòng, insert-select, upsert, `RETURNING`, update-from, delete-using), khoảng 200 toán tử render cho ba dialect, lớp thực thi cho PostgreSQL, MySQL và SQLite, projection theo vị trí kèm `group_children`, đếm và phân trang, decimal, UUID và JSON sau feature flag, percentile `WITHIN GROUP` trên PostgreSQL, và codegen cho SQLite.

Chưa có: codegen cho PostgreSQL và MySQL.

Pre-1.0 nên API bám theo latest stable Rust và có thể thay đổi.

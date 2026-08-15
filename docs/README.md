# Tài liệu OxideR-Query

OxideR-Query là thư viện dựng câu lệnh SQL type-safe, đa dialect cho Rust, lấy cảm hứng từ QueryDSL (Java).
Bạn định nghĩa entity bằng struct Rust thường, derive `Entity`, rồi dựng query mà kiểu của từng cột được kiểm tra ngay lúc biên dịch.
Query được dựng một lần thành AST độc lập dialect, sau đó `render(&dialect)` sinh ra cặp `(sql, params)` cho PostgreSQL, MySQL hoặc SQLite.

Bộ tài liệu này hướng dẫn dùng thư viện từ đầu tới lúc thực thi query trên database thật.

## Mục lục

1. [Bắt đầu nhanh](./01-getting-started.md) - cài đặt, entity đầu tiên, query đầu tiên, chạy thử end-to-end.
2. [Định nghĩa entity và cột](./02-entities-and-columns.md) - `#[derive(Entity)]`, ánh xạ kiểu, cột nullable, kiểu domain tùy biến.
3. [SELECT và lọc dữ liệu](./03-select-and-filtering.md) - projection, `filter`, toán tử so sánh, `and`/`or`, sắp xếp, phân trang, query động.
4. [JOIN và phạm vi bảng ở tầng type](./04-joins.md) - `join`, `left_join`, khóa join type-safe, kiểm tra "đã join chưa".
5. [Aggregate, mutation và subquery](./05-aggregates-mutations-subqueries.md) - `COUNT`/`SUM`/`AVG`/`MIN`/`MAX`, `GROUP BY`/`HAVING`, `INSERT`/`UPDATE`/`DELETE`, `IN`/`EXISTS`.
6. [Dialect, thực thi và codegen](./06-dialects-execution-codegen.md) - render đa dialect, chạy query qua sqlx, sinh entity từ schema có sẵn.
7. [Bảo đảm type-safety](./07-type-safety.md) - những lỗi dùng sai bị bắt lúc biên dịch và thông báo tương ứng.
8. [Tra cứu nhanh API](./08-api-cheatsheet.md) - bảng tổng hợp mọi phương thức và toán tử.

## Cài đặt

Thư viện chưa publish lên crates.io, nên khai báo phụ thuộc qua Git.
Người dùng chỉ cần crate facade `oxider-query`; nó re-export core và derive macro.

```toml
[dependencies]
oxider-query = { git = "https://github.com/OxideR-System/OxideR-Query" }

# Tùy chọn: lớp thực thi async qua sqlx (SQLite hôm nay)
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
    .order_by(User::age.desc())
    .limit(20)
    .render(&Postgres);

// rendered.sql:
//   SELECT "users"."id", "users"."name" FROM "users"
//   WHERE (("users"."name" LIKE $1) AND ("users"."age" >= $2))
//   ORDER BY "users"."age" DESC LIMIT 20
// rendered.params: ["%nguyen%", 18]
```

## Các crate trong workspace

| Crate | Vai trò |
|-------|---------|
| `oxider-query` | Crate facade người dùng phụ thuộc vào. Re-export core + derive macro. |
| `oxider-query-core` | AST, tầng biểu thức type-safe, builder, trait `Dialect`, renderer. Không DB, không macro. |
| `oxider-query-macros` | `#[derive(Entity)]` sinh metamodel. |
| `oxider-query-exec` | Tùy chọn: thực thi async qua sqlx (SQLite hôm nay). Bind param, map row. |
| `oxider-query-codegen` | Tùy chọn: introspect schema, sinh struct `Entity` từ database có sẵn (SQLite hôm nay). |

## Trạng thái

Phiên bản 0.1.0, đang phát triển tích cực.
Core đã đủ tính năng cho SQLite: SELECT type-safe (WHERE, JOIN, aggregate, GROUP BY/HAVING, subquery) và INSERT/UPDATE/DELETE, render cho cả ba dialect, kèm thực thi async và codegen schema trên SQLite.
Pre-1.0 nên API bám theo latest stable Rust và có thể thay đổi.

---
id: codegen
title: 13. Sinh entity từ schema
sidebar_position: 13
---

# 13. Sinh entity từ schema

Với database đã có sẵn, `oxider-query-codegen` đọc schema và sinh ra mã nguồn Rust: mỗi bảng một struct `#[derive(Entity)]`.

Đây là đường "introspection" của metamodel, đối trọng với đường viết tay ở [chương 2](./02-entities-and-columns.md).

## 13.1. Cách dùng

```toml
[dependencies]
oxider-query-codegen = { git = "https://github.com/OxideR-System/OxideR-Query" }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
```

```rust
use sqlx::SqlitePool;

let pool = SqlitePool::connect("sqlite://app.db").await?;
let source = oxider_query_codegen::generate_entities(&pool).await?;
std::fs::write("src/entities.rs", source)?;
```

Kết quả là một chuỗi mã nguồn.
Ghi nó ra file rồi commit vào repo, hoặc sinh trong `build.rs`; thư viện không áp đặt cách nào.

Hôm nay chỉ có SQLite (feature mặc định `sqlite`).

## 13.2. Kết quả trông thế nào

Với schema:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT,
    active BOOLEAN NOT NULL
);
CREATE TABLE order_items (id INTEGER PRIMARY KEY, price REAL NOT NULL);
```

Sinh ra, sắp theo tên bảng:

```rust
#[derive(Entity)]
#[oxider(table = "order_items")]
pub struct OrderItems {
    pub id: i64,
    pub price: f64,
}

#[derive(Entity)]
#[oxider(table = "users")]
pub struct Users {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub active: bool,
}
```

## 13.3. Ánh xạ kiểu

SQLite không có kiểu chặt, nên ánh xạ đi theo quy tắc type affinity của chính nó:

| Kiểu khai báo chứa | Kiểu Rust |
|---|---|
| `INT` | `i64` |
| `CHAR`, `CLOB`, `TEXT` | `String` |
| `BOOL` | `bool` |
| `REAL`, `FLOA`, `DOUB` | `f64` |
| `BLOB`, hoặc không khai báo kiểu | `Vec<u8>` |
| còn lại (`NUMERIC`, `DECIMAL`, `DATE`, ...) | `String` |

Nhóm cuối giữ nguyên dạng text thay vì đoán một kiểu số có thể mất mát.
Nếu bạn biết rõ hơn, hãy sửa lại kiểu trong file đã sinh, hoặc chuyển sang viết entity bằng tay.

Cột không có `NOT NULL` thành `Option<_>`.
Ngoại lệ: cột khóa chính báo `notnull = 0` trong `PRAGMA table_info` nhưng không bao giờ null được, nên mọi cột khóa chính đều là non-null.

## 13.4. Tên không hợp lệ trong Rust

Đây là phần dễ bị bỏ sót nhất, và cũng là phần được test kỹ nhất: mã sinh ra phải **luôn** parse được thành Rust hợp lệ.

Cột trùng từ khóa Rust trở thành raw identifier.
Không cần thuộc tính đổi tên, vì derive macro tự bỏ tiền tố `r#` khi suy ra tên cột:

```sql
CREATE TABLE events (id INTEGER PRIMARY KEY, type TEXT NOT NULL, match TEXT, ref TEXT NOT NULL);
```

```rust
#[derive(Entity)]
#[oxider(table = "events")]
pub struct Events {
    pub id: i64,
    pub r#type: String,
    pub r#match: Option<String>,
    pub r#ref: String,
}
```

Cột không thể thành identifier bằng mọi cách được đổi tên, và tên thật được ghi lại bằng `#[oxider(column = "...")]`.
Thiếu thuộc tính đó thì struct sinh ra sẽ im lặng truy vấn một cột không tồn tại:

```sql
CREATE TABLE metrics (
    id INTEGER PRIMARY KEY,
    "total-count" INTEGER NOT NULL,
    "2fa enabled" BOOLEAN NOT NULL
);
```

```rust
#[derive(Entity)]
#[oxider(table = "metrics")]
pub struct Metrics {
    pub id: i64,
    #[oxider(column = "total-count")]
    pub total_count: i64,
    #[oxider(column = "2fa enabled")]
    pub _2fa_enabled: bool,
}
```

Tên bảng cũng được chuẩn hóa thành tên kiểu PascalCase, tách từ ở mọi ký tự không phải chữ hay số:

| Tên bảng | Tên struct |
|---|---|
| `users` | `Users` |
| `order_items` | `OrderItems` |
| `user-sessions` | `UserSessions` |
| `2fa_codes` | `_2faCodes` |

Vài từ khóa (`crate`, `self`, `Self`, `super`) không dùng được làm raw identifier, nên chúng được thêm hậu tố gạch dưới và ghi lại tên cột thật.

## 13.5. Sau khi sinh

File sinh ra là mã nguồn bình thường, sửa được thoải mái.
Những việc thường phải làm thêm:

- Đổi tên struct cho tự nhiên (`Users` thành `User`), giữ nguyên `#[oxider(table = ...)]`.
- Thay `String` bằng kiểu thời gian thật cho cột ngày giờ.
- Thêm `#[derive(sqlx::FromRow)]` nếu bạn dùng [lớp thực thi](./12-execution.md).
- Thêm `#[oxider(skip)]` cho field tính toán mà bạn thêm vào.

## Bước tiếp theo

[Chương 14](./14-type-safety.md) tổng kết những gì trình biên dịch bảo đảm cho bạn.

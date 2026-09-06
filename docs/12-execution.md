---
id: execution
title: 12. Thực thi query
sidebar_position: 12
---

# 12. Thực thi query

Crate `oxider-query-exec` là lớp tùy chọn nối query với [sqlx].

Core không biết gì về database.
Bạn hoàn toàn có thể tự cầm `(sql, params)` từ `to_sql` rồi chạy bằng driver của mình; crate này chỉ gói lại phần lặp đi lặp lại.

[sqlx]: https://github.com/launchbadge/sqlx

## 12.1. Handle `Db`

```toml
[dependencies]
oxider-query = { git = "https://github.com/OxideR-System/OxideR-Query" }
oxider-query-exec = { git = "https://github.com/OxideR-System/OxideR-Query" }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use oxider_query::prelude::*;
use oxider_query_exec::SqliteDb;

#[derive(Entity, sqlx::FromRow)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i64,
}

let db = SqliteDb::connect("sqlite::memory:").await?;
let users: Vec<User> = db.fetch_all(User::query().filter(User::age.ge(18))).await?;
```

Chú ý là không có `.to_sql(&Sqlite)` ở đâu cả.
`Db` chọn dialect từ backend của chính nó, nên query giữ nguyên tính độc lập engine, và đổi database là đổi một dòng khai báo kiểu handle chứ không phải sửa query.

`SqliteDb` là bí danh của `Db<sqlx::Sqlite>`.

## 12.2. Bốn phương thức chạy query

| Phương thức | Trả về |
|---|---|
| `execute(q)` | `u64`, số dòng bị ảnh hưởng |
| `fetch_all(q)` | `Vec<O>` |
| `fetch_one(q)` | `O`, lỗi nếu không có đúng một dòng |
| `fetch_optional(q)` | `Option<O>` |

`O` là bất kỳ kiểu nào implement `sqlx::FromRow`.
Cách thông thường là derive nó ngay trên chính struct entity.

```rust
db.execute(
    User::insert().set(User::name, "ada").set(User::age, 36),
).await?;

let one: Option<User> = db
    .fetch_optional(User::query().filter(User::name.eq("ada")))
    .await?;
```

`Db::pool()` trả về pool sqlx phía dưới, cho những việc thư viện không lo, ví dụ chạy migration.

## 12.3. Transaction

Cách nên dùng là `transaction`, vì nó không thể quên commit:

```rust
db.transaction(async |tx| {
    tx.execute(User::insert().set(User::id, 1).set(User::age, 30)).await?;
    tx.execute(User::update().set(User::age, 31).filter(User::id.eq(1))).await?;
    Ok(())
})
.await?;
```

Closure trả `Ok` thì commit, trả `Err` thì rollback, và giá trị nó trả về được truyền ra ngoài.
Biên transaction chính là biên của closure, nên mọi lối thoát sớm đều rollback.

Kiểu lỗi chỉ cần `From<oxider_query_exec::Error>`, nên closure trả `oxider_query_exec::Result` dùng được ngay.

Cần điều khiển thủ công thì `begin` trả về một `Tx` có cùng bốn phương thức, kết thúc bằng `commit()` hoặc `rollback()`.
Drop một `Tx` chưa commit sẽ rollback.

## 12.4. Xử lý lỗi

```rust
pub enum Error {
    Render(RenderError),
    Database(sqlx::Error),
}
pub type Result<T> = core::result::Result<T, Error>;
```

Hai nguồn lỗi được tách bạch có chủ đích.

`Render` nghĩa là dialect không diễn đạt được query, và nó xảy ra **trước khi** chạm tới database.
Không có kết nối nào bị mở, không có transaction nào bị bỏ dở.

`Database` là lỗi thật từ engine.

```rust
match db.fetch_all::<User, _>(query).await {
    Ok(rows) => rows,
    Err(oxider_query_exec::Error::Render(e)) => {
        // query không hợp lệ với engine này, sửa query
        return Err(e.into());
    }
    Err(oxider_query_exec::Error::Database(e)) => {
        // lỗi runtime, có thể thử lại
        return Err(e.into());
    }
}
```

`Error` implement `From<RenderError>` và `From<sqlx::Error>`, nên toán tử `?` hoạt động tự nhiên với cả hai.

## 12.5. Bind tham số

`Value` được bind thành kiểu native của driver.
Trên SQLite, kiểu ngày giờ bind thành text, đúng dạng ISO-8601 mà tầng giá trị sinh ra.

Bạn không phải làm gì cả; đây là ghi chú để hiểu vì sao một cột `DATETIME` so sánh được với `NaiveDateTime` mà không cần chuyển đổi thủ công.

## 12.6. Thêm một backend

`Backend` là một trait nhỏ:

```rust
pub trait Backend: Database {
    type Dialect: Dialect + Default;
    fn bind(...) -> ...;
    fn bind_as(...) -> ...;
    fn rows_affected(result: ...) -> u64;
}
```

Implement nó cho một `sqlx::Database` là đủ để `Db` và `Tx` chạy trên backend đó, không phải sửa gì trong hai kiểu ấy.

Hôm nay chỉ có SQLite (feature mặc định `sqlite`).
PostgreSQL và MySQL lắp vào theo đúng đường này.

## Bước tiếp theo

[Chương 13](./13-codegen.md) sinh entity từ một schema đã có.

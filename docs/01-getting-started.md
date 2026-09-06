---
id: getting-started
title: 1. Bắt đầu nhanh
sidebar_position: 1
---

# 1. Bắt đầu nhanh

Chương này đưa bạn từ dự án trống tới một query chạy được trên database thật.

## 1.1. Thêm phụ thuộc

Trong `Cargo.toml`:

```toml
[dependencies]
oxider-query = { git = "https://github.com/OxideR-System/OxideR-Query" }
```

Import qua module `prelude`, nơi gom sẵn mọi thứ hay dùng:

```rust
use oxider_query::prelude::*;
```

`prelude` mang vào derive macro `Entity`, ba dialect `Postgres`/`MySql`/`Sqlite`, các hàm dựng biểu thức (`val`, `col`, `case_when`, `coalesce`, `count_all`, `row_number`, ...), và toàn bộ trait toán tử.
Các trait toán tử phải nằm trong scope thì method của chúng mới resolve được, đó là lý do `prelude` tồn tại thay vì bắt bạn import từng cái.

## 1.2. Định nghĩa entity đầu tiên

Một entity là một struct thường với các field có tên.
Derive `Entity` và chỉ tên bảng bằng thuộc tính `#[oxider(table = "...")]`.

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
```

Macro sinh cho mỗi field một associated const cùng tên: `User::id`, `User::name`, `User::age`, `User::email`.
Đây chính là metamodel.
Không có kiểu `QUser` sinh ra ở thư mục khác phải import riêng như QueryDSL.

## 1.3. Dựng query đầu tiên

Bắt đầu bằng `User::query()`, nối các mệnh đề, kết thúc bằng `to_sql(&dialect)`:

```rust
let rendered = User::query()
    .filter(User::age.ge(18))
    .order_by(User::name.asc())
    .to_sql(&Postgres)
    .unwrap();

assert_eq!(
    rendered.sql,
    r#"SELECT * FROM "users" WHERE "users"."age" >= $1 ORDER BY "users"."name" ASC"#
);
assert_eq!(rendered.params.len(), 1);
```

Không gọi `select` thì projection mặc định là `SELECT *`.
Muốn liệt kê cột cụ thể, dùng `.select(...)`, xem [mục 3.1](./03-select-and-filtering.md).

`to_sql` trả về `Result<Rendered, RenderError>`:

```rust
pub struct Rendered {
    pub sql: String,
    pub params: Vec<Value>,
}
```

`sql` là chuỗi SQL với placeholder của dialect.
`params` là danh sách giá trị bind theo đúng thứ tự xuất hiện.
Thư viện không bao giờ nối giá trị vào chuỗi SQL, nên không có đường cho SQL injection.

Vì sao `to_sql` trả `Result`: không phải engine nào cũng diễn đạt được mọi thứ.
`FULL JOIN` trên MySQL, `DISTINCT ON` ngoài PostgreSQL, `FOR UPDATE` trên SQLite đều bị từ chối tại đây kèm thông báo nói rõ dialect nào và cấu trúc nào.
Chi tiết ở [chương 11](./11-dialects.md).

## 1.4. Đổi dialect không đổi query

Cùng một query render sang dialect khác chỉ khác cách trích dẫn định danh và kiểu placeholder:

```rust
let query = User::query().filter(User::age.ge(18));

query.to_sql(&Postgres).unwrap().sql;
//   SELECT * FROM "users" WHERE "users"."age" >= $1

query.to_sql(&MySql).unwrap().sql;
//   SELECT * FROM `users` WHERE `users`.`age` >= ?

query.to_sql(&Sqlite).unwrap().sql;
//   SELECT * FROM "users" WHERE "users"."age" >= ?
```

`to_sql` nhận `&self`, nên một query dựng sẵn render được cho nhiều dialect mà không cần dựng lại.

## 1.5. Chạy query trên database thật

Thêm crate thực thi và sqlx.
Struct nhận kết quả cần derive `sqlx::FromRow`.

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
    active: bool,
}

#[tokio::main]
async fn main() -> oxider_query_exec::Result<()> {
    // Một handle Db duy nhất, tự biết dialect theo backend.
    let db = SqliteDb::connect("sqlite::memory:").await?;
    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER, active BOOLEAN)")
        .execute(db.pool())
        .await?;

    // INSERT: không phải truyền dialect, handle tự chọn.
    db.execute(
        User::insert()
            .set(User::name, "Alice")
            .set(User::age, 30)
            .set(User::active, true),
    )
    .await?;

    // SELECT rồi map row về struct.
    let adults: Vec<User> = db
        .fetch_all(
            User::query()
                .filter(User::age.ge(18))
                .order_by(User::age.asc()),
        )
        .await?;

    println!("{} người lớn", adults.len());
    Ok(())
}
```

Chi tiết lớp thực thi, transaction và xử lý lỗi ở [chương 12](./12-execution.md).

## 1.6. Ba lỗi trình biên dịch bạn sẽ gặp sớm

Đây là điểm khác biệt lớn nhất so với việc viết SQL bằng chuỗi.
Cả ba lỗi dưới đây đều xảy ra lúc biên dịch, không phải lúc chạy.

So sánh cột với sai kiểu:

```rust
User::name.eq(123);
// error: the trait bound `i32: IntoExpr<String>` is not satisfied
```

Tham chiếu bảng chưa join:

```rust
User::query().filter(Department::name.eq("AI"));
// error: the trait bound `Nil: Contains<Department, _>` is not satisfied
```

Aggregate không áp dụng được cho kiểu cột:

```rust
Order::status.sum();
// error: the trait bound `String: Numeric` is not satisfied
```

Danh sách đầy đủ những gì được bảo đảm ở [chương 14](./14-type-safety.md).

## Bước tiếp theo

- Hiểu sâu cách khai báo entity và ánh xạ kiểu: [chương 2](./02-entities-and-columns.md).
- Dựng query phức tạp hơn: [chương 3](./03-select-and-filtering.md).

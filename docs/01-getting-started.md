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

`prelude` mang vào: derive macro `Entity`, kiểu `Column`, `Predicate`, `OrderTerm`, ba dialect `Postgres`/`MySql`/`Sqlite`, các hàm aggregate (`count`, `count_all`, `sum`, `avg`, `min`, `max`), `exists`/`not_exists`, và các trait `Dialect`, `ToSqlValue`.

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
Đây chính là "metamodel"; không có kiểu Q-type riêng phải đặt tên như QueryDSL.

## 1.3. Dựng query đầu tiên

Bắt đầu bằng `User::query()`, nối các mệnh đề, kết thúc bằng `render(&dialect)`:

```rust
let rendered = User::query()
    .filter(User::age.ge(18))
    .order_by(User::name.asc())
    .render(&Postgres);

assert_eq!(
    rendered.sql,
    r#"SELECT * FROM "users" WHERE ("users"."age" >= $1) ORDER BY "users"."name" ASC"#
);
```

Không gọi `select` thì projection mặc định là `SELECT *`.
Muốn liệt kê cột cụ thể, dùng `.select(...)` ở [mục 3.2](./03-select-and-filtering.md#32-chọn-cột-projection).

`render` trả về một `Rendered`:

```rust
pub struct Rendered {
    pub sql: String,
    pub params: Vec<Value>,
}
```

`sql` là chuỗi SQL với placeholder của dialect; `params` là danh sách giá trị bind theo đúng thứ tự.
Thư viện không tự nối giá trị vào SQL nên không có nguy cơ SQL injection.

## 1.4. Đổi dialect không đổi query

Cùng một query render sang dialect khác chỉ khác cách trích dẫn định danh và kiểu placeholder:

```rust
User::query().filter(User::age.ge(18)).render(&MySql).sql;
//   SELECT * FROM `users` WHERE (`users`.`age` >= ?)

User::query().filter(User::age.ge(18)).render(&Sqlite).sql;
//   SELECT * FROM "users" WHERE ("users"."age" >= ?)
```

Lưu ý: `Select` tiêu thụ chính nó khi render (nhận `self`), nên nếu cần render nhiều dialect thì dựng lại query hoặc tách phần dùng chung.

## 1.5. Chạy query trên database thật

Thêm crate thực thi và sqlx.
Row cần derive `sqlx::FromRow` để map về struct.

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
async fn main() -> Result<(), sqlx::Error> {
    // Một handle Db duy nhất, tự biết dialect theo backend.
    let db = SqliteDb::connect("sqlite::memory:").await?;
    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER, active BOOLEAN)")
        .execute(db.pool())
        .await?;

    // INSERT - không có .render(&Sqlite): handle tự chọn dialect.
    db.execute(
        User::insert().value(User::name, "Alice").value(User::age, 30).value(User::active, true),
    )
    .await?;

    // SELECT + map row -> struct
    let adults: Vec<User> = db
        .fetch_all(User::query().filter(User::age.ge(18)).order_by(User::age.asc()))
        .await?;

    println!("{} người lớn", adults.len());
    Ok(())
}
```

Chi tiết lớp thực thi ở [chương 6](./06-dialects-execution-codegen.md).

## Bước tiếp theo

- Muốn hiểu sâu cách khai báo entity và ánh xạ kiểu: [chương 2](./02-entities-and-columns.md).
- Muốn dựng query phức tạp hơn (lọc, sắp xếp, projection, query động): [chương 3](./03-select-and-filtering.md).

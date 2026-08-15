# 6. Dialect, thực thi và codegen

Chương này bàn ba lớp quanh builder: render đa dialect, thực thi query trên database, và sinh entity từ schema có sẵn.

## 6.1. Render và dialect

Query dựng thành AST độc lập dialect; `render(&dialect)` mới sinh SQL cụ thể.
Có sẵn ba dialect, khác nhau ở cách trích dẫn định danh và kiểu placeholder:

| Dialect | Định danh | Placeholder |
|---------|-----------|-------------|
| `Postgres` | `"ident"` | `$1, $2, ...` |
| `MySql` | `` `ident` `` | `?` |
| `Sqlite` | `"ident"` | `?` |

```rust
let q = User::query().filter(User::age.ge(18));
q.render(&Postgres).sql; // ... WHERE ("users"."age" >= $1)

User::query().filter(User::age.ge(18)).render(&MySql).sql;
// ... WHERE (`users`.`age` >= ?)
```

`render` trả về `Rendered { sql: String, params: Vec<Value> }`.
Bạn có thể tự bind `params` vào driver bất kỳ, hoặc dùng crate `oxider-query-exec` bên dưới.

### Tự viết dialect

`Dialect` là một trait nhỏ:

```rust
pub trait Dialect {
    fn quote_ident(&self, ident: &str) -> String;
    fn placeholder(&self, index: usize) -> String;
}
```

Muốn hỗ trợ một database khác, implement trait này cho một struct của bạn rồi truyền vào `render`.

## 6.2. Thực thi qua `oxider-query-exec`

Core không tự quản kết nối.
Crate `oxider-query-exec` cầu nối `Rendered` sang [sqlx](https://github.com/launchbadge/sqlx) và chạy statement.
Hôm nay hỗ trợ SQLite (feature `sqlite`, bật mặc định); Postgres và MySQL sẽ theo cùng khuôn sau này.

Bốn hàm async, đều generic trên executor sqlx (nhận `&Pool`, `&mut Connection`, hoặc transaction).
Quan trọng: chúng nhận thẳng **query builder**, không phải chuỗi đã render.
Lớp exec tự render bằng dialect của database đang kết nối, nên **call site không còn `.render(&Sqlite)`**:

| Hàm | Trả về | Dùng cho |
|-----|--------|----------|
| `execute(exec, query)` | `u64` (số dòng ảnh hưởng) | INSERT/UPDATE/DELETE |
| `fetch_all(exec, query)` | `Vec<O>` | SELECT nhiều dòng |
| `fetch_one(exec, query)` | `O` | SELECT đúng một dòng |
| `fetch_optional(exec, query)` | `Option<O>` | SELECT không hoặc một dòng |

Tham số `query` nhận bất cứ thứ gì implement `Renderable`: `Select`, `Insert`, `Update`, `Delete`, hoặc một `Rendered` dựng sẵn.
Kiểu kết quả `O` phải implement `sqlx::FromRow`.
Thường bạn derive cả `Entity` lẫn `sqlx::FromRow` trên cùng struct:

```rust
use oxider_query::prelude::*;
use sqlx::SqlitePool;

#[derive(Entity, sqlx::FromRow)]
#[oxider(table = "users")]
struct User { id: i64, name: String, age: i64, active: bool }

async fn run(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Ghi - không có .render(&...) ở đây
    let affected = oxider_query_exec::execute(
        pool,
        User::insert().value(User::name, "Alice").value(User::age, 30).value(User::active, true),
    ).await?;
    assert_eq!(affected, 1);

    // Đọc nhiều
    let adults: Vec<User> = oxider_query_exec::fetch_all(
        pool,
        User::query().filter(User::age.ge(18)).order_by(User::age.asc()),
    ).await?;

    // Đọc một (có thể không có)
    let maybe: Option<User> = oxider_query_exec::fetch_optional(
        pool,
        User::query().filter(User::id.eq(1)),
    ).await?;

    let _ = (adults, maybe);
    Ok(())
}
```

Vì query giữ nguyên dạng dialect-agnostic và dialect chỉ được chọn bên trong lớp exec (theo loại pool), đổi sang Postgres/MySQL sau này chỉ là đổi loại connection pool - **không sửa một dòng query nào**.
Đây là cách khuyến nghị để tránh rải `.render(&Postgres)` khắp code.

`Value` được bind sang kiểu sqlx tương ứng: `Bool -> bool`, `Int -> i64`, `Real -> f64`, `Text -> String`, `Null -> NULL`.

Nếu cần tự cầm SQL (log, driver khác, dialect tùy biến), bạn vẫn gọi `.render(&dialect)` để lấy `Rendered { sql, params }` rồi xử lý tay; lớp exec chỉ là tiện ích phía trên.

### Cargo cho lớp exec

```toml
[dependencies]
oxider-query = { git = "https://github.com/OxideR-System/OxideR-Query" }
oxider-query-exec = { git = "https://github.com/OxideR-System/OxideR-Query" } # default-feature "sqlite"
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 6.3. Sinh entity từ schema với `oxider-query-codegen`

Nếu đã có database, bạn không cần gõ tay struct entity.
`oxider-query-codegen` introspect schema và sinh mã nguồn struct kèm `#[derive(Entity)]`.
Hôm nay hỗ trợ SQLite.

```rust
use sqlx::SqlitePool;

async fn dump(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let source: String = oxider_query_codegen::generate_entities(pool).await?;
    println!("{source}");
    Ok(())
}
```

Với bảng:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT,
    active BOOLEAN NOT NULL
);
```

hàm sinh ra:

```rust
#[derive(Entity)]
#[oxider(table = "users")]
pub struct Users {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub active: bool,
}
```

Quy tắc introspect:

- Tên struct = tên bảng dạng PascalCase; các bảng sinh theo thứ tự tên.
- Kiểu cột suy từ type affinity SQLite: `INTEGER -> i64`, `TEXT/CHAR/CLOB -> String`, `REAL/FLOAT/DOUBLE -> f64`, `BOOLEAN -> bool`, `BLOB` hoặc trống `-> Vec<u8>`.
- Cột `NOT NULL` hoặc khóa chính `-> T`; cột nullable khác `-> Option<T>`.
  Khóa chính kiểu rowid (`INTEGER PRIMARY KEY`) luôn được coi là non-null.

Kết quả là `String` mã nguồn; bạn ghi ra file trong build script hoặc dán vào crate.

## Bước tiếp theo

Xem [chương 7](./07-type-safety.md) để hiểu trọn bộ bảo đảm type-safety, hoặc [chương 8](./08-api-cheatsheet.md) để tra cứu nhanh.

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
Crate `oxider-query-exec` cung cấp một handle `Db` bọc một sqlx pool, cầu nối query sang [sqlx](https://github.com/launchbadge/sqlx) và chạy statement.
Hôm nay hỗ trợ SQLite (feature `sqlite`, bật mặc định, alias `SqliteDb = Db<Sqlite>`); Postgres và MySQL sẽ theo cùng khuôn sau này.

`Db` là **một API duy nhất cho mọi backend**: nó tự biết dialect theo backend, nên **call site không còn `.render(&Sqlite)`**.
Tạo handle bằng `Db::connect(url)` (hoặc `Db::new(pool)` nếu đã có sẵn pool), rồi gọi các method:

| Method | Trả về | Dùng cho |
|--------|--------|----------|
| `db.execute(query)` | `u64` (số dòng ảnh hưởng) | INSERT/UPDATE/DELETE |
| `db.fetch_all(query)` | `Vec<O>` | SELECT nhiều dòng |
| `db.fetch_one(query)` | `O` | SELECT đúng một dòng |
| `db.fetch_optional(query)` | `Option<O>` | SELECT không hoặc một dòng |

Tham số `query` nhận bất cứ thứ gì implement `Renderable`: `Select`, `Insert`, `Update`, `Delete`, hoặc một `Rendered` dựng sẵn.
Kiểu kết quả `O` phải implement `sqlx::FromRow`.
Thường bạn derive cả `Entity` lẫn `sqlx::FromRow` trên cùng struct:

```rust
use oxider_query::prelude::*;
use oxider_query_exec::SqliteDb;

#[derive(Entity, sqlx::FromRow)]
#[oxider(table = "users")]
struct User { id: i64, name: String, age: i64, active: bool }

async fn run(db: &SqliteDb) -> Result<(), sqlx::Error> {
    // Ghi - không có .render(&...) ở đây
    let affected = db.execute(
        User::insert().value(User::name, "Alice").value(User::age, 30).value(User::active, true),
    ).await?;
    assert_eq!(affected, 1);

    // Đọc nhiều
    let adults: Vec<User> = db
        .fetch_all(User::query().filter(User::age.ge(18)).order_by(User::age.asc()))
        .await?;

    // Đọc một (có thể không có)
    let maybe: Option<User> = db
        .fetch_optional(User::query().filter(User::id.eq(1)))
        .await?;

    let _ = (adults, maybe);
    Ok(())
}
```

Vì query giữ nguyên dạng dialect-agnostic và dialect chỉ được chọn bên trong `Db` (theo backend), đổi sang Postgres/MySQL sau này chỉ là đổi kiểu handle (`SqliteDb` -> `PostgresDb`) - **không sửa một dòng query nào**.
Đây là cách khuyến nghị để tránh rải `.render(&Postgres)` khắp code.

Cần thao tác sqlx thô (query tay ngoài builder) thì `db.pool()` trả về `&Pool` bên dưới.

`Value` được bind sang kiểu sqlx tương ứng: `Bool -> bool`, `Int -> i64`, `Real -> f64`, `Text -> String`, `Null -> NULL`.

### Transaction

`db.begin()` mở một transaction, trả về handle `Tx` có **đúng bộ method như `Db`** (`execute`/`fetch_all`/`fetch_one`/`fetch_optional`).
Kết thúc bằng `tx.commit()` (ghi bền) hoặc `tx.rollback()` (hủy).
Bỏ handle mà không commit thì tự rollback.

```rust
let mut tx = db.begin().await?;
tx.execute(User::insert().value(User::id, 1).value(User::name, "Alice")).await?;
tx.execute(User::update().set(User::age, 31).filter(User::id.eq(1))).await?;

// Đọc trong transaction thấy được thay đổi chưa commit của chính nó.
let pending: Vec<User> = tx.fetch_all(User::query()).await?;

tx.commit().await?; // hoặc tx.rollback().await? để hủy toàn bộ
```

Lưu ý: method của `Tx` nhận `&mut self` (transaction cần truy cập độc quyền), nên biến `tx` phải khai báo `mut`.
Query truyền vào vẫn dialect-agnostic y hệt, `Tx` tự render theo backend.

#### Transaction có phạm vi: `db.transaction(...)`

`begin`/`commit`/`rollback` thủ công linh hoạt nhưng dễ quên `commit`, hoặc quên rollback khi có `?` trả sớm.
`db.transaction(closure)` đóng khung việc đó: chạy closure trong transaction, **`commit` nếu closure trả `Ok`, `rollback` nếu trả `Err`** (kể cả `?` bail sớm).

```rust
let inserted: usize = db
    .transaction(async |tx| {
        tx.execute(User::insert().value(User::id, 1).value(User::age, 30)).await?;
        tx.execute(User::update().set(User::age, 31).filter(User::id.eq(1))).await?;
        let rows: Vec<User> = tx.fetch_all(User::query()).await?;
        Ok::<_, sqlx::Error>(rows.len())
    })
    .await?;
```

Closure nhận `&mut Tx` và trả `Result<T, E>`; giá trị `T` được trả ra ngoài.
`E` chỉ cần `From<sqlx::Error>` (cho bước begin/commit/rollback), nên closure trả `Result<_, sqlx::Error>` dùng thẳng được.
Đây là kiểu khuyến nghị cho luồng "commit khi xong, rollback khi lỗi" thường gặp - không thể quên commit.

> So với QueryDSL: QueryDSL không tự quản transaction, nó giao cho framework xung quanh.
> Với Spring, `SQLQueryFactory` lấy connection qua `SpringConnectionProvider` (connection đã gắn transaction), còn ranh giới transaction do `@Transactional` bọc quanh một method - commit/rollback tự động theo method đó.
> `db.transaction(closure)` chính là bản Rust của scope đó: ranh giới là closure thay vì annotation, commit/rollback tự động theo kết quả.
> Cần điều khiển tay từng bước thì dùng `db.begin()` như trên.

Nếu cần tự cầm SQL (log, driver khác, dialect tùy biến), bạn vẫn gọi `.render(&dialect)` để lấy `Rendered { sql, params }` rồi xử lý tay; `Db` chỉ là tiện ích phía trên.

### Thêm backend mới

`Db<DB>` generic trên backend qua trait `Backend`, ánh xạ một sqlx `Database` tới dialect của nó cộng cách bind param.
Thêm Postgres/MySQL = implement `Backend` cho sqlx `Postgres`/`MySql` sau feature tương ứng, không đụng gì tới `Db`.

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

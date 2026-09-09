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
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }  # hoặc "postgres"
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
`PostgresDb` là bí danh của `Db<sqlx::Postgres>`, sau feature `postgres`, và mọi phương thức dưới đây giống hệt nhau cho cả hai.
Đổi backend là đổi kiểu của handle, không đụng tới một dòng query nào.

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
Bạn không phải làm gì cả; mục này giải thích vì sao một cột `DATE` so sánh được với `NaiveDate` mà không cần chuyển đổi thủ công, và vì sao chỗ đó lại khác nhau giữa hai backend.

`Value` lưu ngày giờ dưới dạng text, vì AST không được phụ thuộc vào thư viện ngày tháng nào.
SQLite nhận thẳng: nó quyết định kiểu của một cột theo giá trị được đưa vào, nên chuỗi ISO-8601 là đủ.

PostgreSQL thì không.
Giao thức extended query mang theo một type OID cho từng tham số, sqlx khai báo `text` cho chuỗi Rust, và server từ chối dùng nó ở chỗ cần `date`:

```text
ERROR 42804: column "on_day" is of type date but expression is of type text
```

Nên backend PostgreSQL parse ngược chuỗi về kiểu `chrono` trước khi bind, dùng đúng các hằng format mà tầng giá trị đã ghi ra (`oxider_query_core::formats`).
Ghi và đọc là hai nửa của cùng một hợp đồng, nên chúng dùng chung một định nghĩa thay vì mỗi bên tự khai một bản.

Một điểm cần biết nếu bạn dùng cột có múi giờ: `DateTime<Utc>` được ghi kèm offset tường minh, nên khi vào cột `TIMESTAMPTZ` nó không bị diễn giải lại theo múi giờ của session.

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

Hôm nay có SQLite (feature mặc định `sqlite`), PostgreSQL (feature `postgres`) và MySQL (feature `mysql`).

Mỗi backend dài khoảng 60 dòng, gần hết là bảng `match` trên `Value`, đúng như trait này hứa hẹn.
Bộ test end-to-end của chúng tự bỏ qua khi biến môi trường tương ứng chưa được đặt, nên `cargo test` mặc định không cần server:

```bash
make pg-up      # dựng một PostgreSQL tạm bằng Docker
make test-pg    # chạy bộ test end-to-end
make pg-down    # xóa nó đi

make mysql-up test-mysql mysql-down   # y hệt, cho MySQL
```

## 12.7. MySQL và mốc thời gian có múi giờ

MySQL không có kiểu datetime mang múi giờ.
`DATETIME` là đồng hồ treo tường không kèm zone, còn `TIMESTAMP` lưu theo UTC nhưng quy đổi cả lúc ghi lẫn lúc đọc theo `time_zone` của kết nối.

Nên một `DateTime<Utc>` khi bind sẽ mất phần offset, chỉ còn **đồng hồ UTC** của nó.
Ghi vào `DATETIME` thì đúng chính xác.
Ghi vào `TIMESTAMP` thì chỉ đúng khi `time_zone` của kết nối là UTC, vì nếu không server sẽ đọc dãy số đó như giờ địa phương rồi dịch đi.

Crate này không tự đặt `time_zone` cho bạn: pool do sqlx dựng và `Db` không chen vào lúc mở kết nối.
Hoặc dùng `DATETIME` cho mốc thời gian, hoặc tự đặt zone cho session:

```sql
SET time_zone = '+00:00'
```

## 12.8. Projection: đọc hàng theo vị trí

`fetch_all` map hàng bằng `sqlx::FromRow`, tức là **khớp theo tên cột**.
Cách đó buộc mọi biểu thức trong `select` phải có alias, và sai tên chỉ lộ ra khi có hàng trả về.

`#[derive(Projection)]` làm ngược lại: field thứ *n* đọc cột thứ *n* trong **span** của nó, tức là bám theo thứ tự của `select`.

```rust
#[derive(Entity, Projection)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    age: i64,
}

let users: Vec<User> = db
    .fetch_all_projected(User::query().select((User::id, User::name, User::age)))
    .await?;
```

Không alias nào cả. Đổi thứ tự trong `select` thì đổi luôn field nào nhận cột nào.

Mã sinh ra gọi tên `oxider-query-exec` chứ không phải facade, vì projection chỉ có nghĩa khi có thứ đang chạy query.

`#[oxider(skip)]` không ăn cột nào và được điền bằng `Default::default()`, vì hàng không mang gì cho nó cả.

### Span ghép được

Vì mỗi projection biết mình rộng bao nhiêu cột, một tuple projection chia một hàng phẳng thành nhiều struct: phần tử sau bắt đầu ở chỗ phần tử trước kết thúc, và các bề rộng được cộng ở tầng type chứ không đọc từ hàng.

```rust
let rows: Vec<(User, Option<Order>)> = db
    .fetch_all_projected(
        User::query()
            .left_join(Order::table(), Order::user_id.eq(User::id))
            .select((User::id, User::name, User::age,
                     Order::id, Order::user_id, Order::total))
            .order_by(User::id.asc()),
    )
    .await?;
```

`Option<P>` là `None` khi **mọi** cột trong span của nó đều NULL, đúng thứ một `LEFT JOIN` không khớp sinh ra.
Nếu chỉ một phần span là NULL thì đó là hàng thật có cột nullable, nên nó vẫn `Some`.

### Điều KHÔNG được kiểm tra

Bề rộng của projection có khớp với `select` hay không thì **không** được kiểm tra lúc biên dịch.
`Select` xoá projection thành `Vec<Node>` ngay khi dựng xong, nên không còn kiểu nào để đối chiếu.
Lệch nhau sẽ hiện ra thành lỗi chỉ số cột từ hàng đầu tiên, không phải lỗi biên dịch.

Muốn kiểm tra được thì `Select` phải mang thêm một tham số kiểu thứ tư xuyên qua mọi method của nó, đắt hơn giá trị nó mang lại ở thời điểm này.

## 12.9. `group_children`: gấp một-nhiều thành cây

Một join một-nhiều trả về cha lặp lại một lần cho mỗi con.
`group_children` gấp nó lại, không cần query thứ hai cho mỗi cha.

```rust
let tree: Vec<(User, Vec<Order>)> = group_children(rows, |user| user.id);
```

Cha giữ nguyên thứ tự lần đầu xuất hiện, con giữ nguyên thứ tự đến.
Trả `Vec` chứ không phải `HashMap` chính là vì thế: `ORDER BY` mà query đã bỏ công xin không được phép mất trong lúc gấp.

Cha có `Vec` rỗng khi `LEFT JOIN` không khớp gì.

Hàm này nhận kết quả đã fetch chứ không gắn vào `Db`, nên ai render rồi tự bind bằng driver của mình vẫn dùng được.

## Bước tiếp theo

[Chương 13](./13-codegen.md) sinh entity từ một schema đã có.

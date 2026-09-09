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

## 12.10. Đếm và phân trang

Phân trang cần hai câu trả lời: lấy hàng nào, và tổng cộng có bao nhiêu.
Tự viết query đếm bằng tay nghĩa là phải giữ nó đồng bộ với query thật.

```rust
let total: u64 = db.fetch_count(User::query().filter(User::age.ge(18))).await?;
```

`count()` **bọc** query lại chứ không thay projection thành `COUNT(*)`:

```sql
SELECT COUNT(*) FROM (SELECT ... ) AS "oxider_count"
```

Bọc mới đúng với `DISTINCT`, `GROUP BY` và set operation, vì ở những query đó số hàng trả về không phải số hàng mệnh đề FROM sinh ra.

`LIMIT` và `OFFSET` bị bỏ: chúng chọn một trang, mà đếm là để biết có bao nhiêu trang.
`ORDER BY` cũng bỏ, vì sắp xếp không đổi được số đếm.
Ngoại lệ duy nhất là `DISTINCT ON`: PostgreSQL bắt buộc biểu thức của nó phải khớp các term `ORDER BY` đầu tiên, nên ở đó thứ tự được giữ lại.

`fetch_page` làm cả hai việc và trả về `Page<T>`:

```rust
let page: Page<User> = db
    .fetch_page(User::query().order_by(User::id.asc()), 1, 20)
    .await?;

page.items;          // 20 hàng của trang thứ hai
page.total;          // tổng số hàng của query chưa phân trang
page.total_pages();  // total chia lên
page.has_next();
```

Trang đánh số từ 0. Có bản `fetch_page_projected` đọc theo vị trí.

Hai lượt đi về, cố ý.
`COUNT(*) OVER ()` làm được trong một lượt nhưng khi trang vượt quá cuối thì không trả hàng nào cả, tức là mất luôn tổng số - đúng lúc người gọi cần nó nhất.

## 12.11. Decimal, UUID và JSON

Ba kiểu này nằm sau feature `rust_decimal`, `uuid` và `json`.
Bật trên `oxider-query` (hoặc `oxider-query-core`) và trên `oxider-query-exec`.

```rust
#[derive(Entity)]
#[oxider(table = "invoices")]
struct Invoice {
    id: uuid::Uuid,
    total: rust_decimal::Decimal,
    metadata: serde_json::Value,
}
```

`Decimal` là `Numeric`, nên có đủ số học và `SUM`/`AVG`.
`Uuid` sắp xếp được, vì Postgres sắp được kiểu `uuid` và keyset paging theo một khóa UUID là chuyện có thật.
`serde_json::Value` chỉ có `SqlType`: JSON không có thứ tự toàn phần và không có số học, cho `<` hay `SUM` là hứa một thứ mà ba engine không đồng ý với nhau.

Một điều không hiển nhiên: số nguyên không tự nới rộng thành `Decimal`.
`Invoice::total.mul(2)` không biên dịch được, phải viết `Decimal::from(2)`.
Đây là cố ý, giống hệt cách `f64` đối xử: danh sách nới rộng được giữ hẹp để suy kiểu còn quyết định được ở những chỗ khác.

### Giá trị đi qua AST dưới dạng text

`Value` mang cả ba dưới dạng text chuẩn hóa, đúng cách nó đang mang date và time.
Lõi nhờ thế không phụ thuộc thư viện decimal, UUID hay JSON nào, và **hình dạng của enum không đổi theo feature**: bật hay tắt, backend vẫn match trên đúng bấy nhiêu nhánh.

Backend đọc text đó ngược lại trước khi bind. Quy tắc:

**Bind bằng kiểu của engine ở nơi engine có kiểu đó, bind bằng text ở nơi không có.**

| | PostgreSQL | MySQL | SQLite |
|---|---|---|---|
| decimal | `NUMERIC` | `DECIMAL` | text |
| UUID | `uuid` | text | text |
| JSON | `jsonb` | `JSON` | text |

UUID trên MySQL trông như ngoại lệ nhưng không phải.
MySQL không có kiểu UUID, nên cột thường là `CHAR(36)`, mà `Uuid` của sqlx mã hóa thành `BINARY(16)`.
Chọn kiểu đó là lặng lẽ ghi mấy byte không đọc được vào mọi cột `CHAR(36)`.
Ai thật sự dùng `BINARY(16)` thì bind `Uuid::as_bytes` như blob - đó là lựa chọn của người gọi chứ không phải mặc định của thư viện.

### Decimal trên SQLite: chọn một trong hai

SQLite không có kiểu decimal chính xác.
Affinity của cột quyết định bạn được gì:

- **`NUMERIC`**: text thành số, so sánh và sắp xếp đúng nghĩa số học.
  Giá trị rộng quá integer thành `REAL`, tức là float, tức là không chính xác nữa.
- **`TEXT`**: giữ nguyên từng chữ số, nhưng mọi so sánh là so sánh chuỗi, nên `"9.5" > "10.25"`.

Không có lựa chọn thứ ba. Postgres và MySQL đều có kiểu thật nên không phải chọn.
Lưu tiền trong SQLite thì nên để đơn vị nhỏ nhất trong `INTEGER`.

Không có gì trong crate này làm việc chuyển đổi đó: decimal rời khỏi đây vẫn là các chữ số của nó.
Cái chuyển đổi là cột.

## 12.12. Duyệt kết quả mà không gom vào bộ nhớ

`fetch_all` dựng một `Vec`. Với một bản xuất dữ liệu, một migration, hay một báo cáo quét cả bảng, cái `Vec` đó chính là vấn đề.

`for_each_row` đưa từng dòng cho closure ngay khi nó về, không gom:

```rust
let mut total = 0i64;
let rows = db.for_each_row(Order::query(), |order: Order| {
    total += order.amount;
    Ok(())
}).await?;
```

Trả về số dòng đã đi qua. Closure trả `Err` thì dừng ngay tại đó và `Err` đó là kết quả - dùng để bỏ ngang mà không phải đọc nốt phần còn lại.
Bản đọc theo vị trí là `for_each_row_projected`, giống quan hệ giữa `fetch_all` và `fetch_all_projected`.
Cả hai đều có trên `Tx`; trong transaction thì vòng duyệt giữ connection bao lâu thì transaction mở bấy lâu.

### Vì sao là fold chứ không phải `Stream`

Một `Stream` phải vừa sở hữu câu SQL đã render vừa mượn từ chính nó, tức là một kiểu tự tham chiếu, hoặc một macro generator từ crate khác.
Cả hai đều không đáng, khi lý do người ta cần streaming ngay từ đầu là để **không** giữ dữ liệu.
Ai thật sự cần một `Stream` để ghép với `futures` thì lái sqlx trực tiếp qua [`Db::pool`].

### Còn "batch DML" thì đã có sẵn

Không có API mới cho nó, vì hai nửa của nhu cầu đó đều đã được phục vụ:

- Nhiều dòng một câu lệnh: `Insert::values(...)` gọi nhiều lần, hoặc insert-select. Xem [chương 10](./10-dml.md).
- Nhiều câu lệnh một lượt: `db.transaction(|tx| ...)`, một vòng lặp trên `tx.execute`.

Thêm một `execute_many` chỉ là gói lại vòng lặp đó, không tiết kiệm được lượt đi về nào.

## Bước tiếp theo

[Chương 13](./13-codegen.md) sinh entity từ một schema đã có.

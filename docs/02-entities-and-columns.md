---
id: entities-and-columns
title: 2. Entity và cột
sidebar_position: 2
---

# 2. Entity và cột

Entity là điểm khởi đầu của mọi query.
Chương này mô tả đầy đủ `#[derive(Entity)]`: nó sinh ra gì, ánh xạ kiểu ra sao, xử lý tên khó thế nào, và cách đặt alias cho bảng.

## 2.1. Macro derive

```rust
use oxider_query::prelude::*;

#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
}
```

Derive `Entity` làm hai việc.

1. Impl trait `Entity` cho struct, cung cấp `User::query()`, `User::query_as("u")`, `User::insert()`, `User::update()`, `User::delete()`, `User::table()`, và hằng `User::TABLE`, `User::SCHEMA`.
2. Sinh một associated const `Column<User, T>` cho mỗi field, tên const trùng tên field.

Với ví dụ trên bạn có `User::id: Column<User, i64>`, `User::name: Column<User, String>`, `User::email: Column<User, String>` với cờ nullable bật.

Mỗi `Column` mang hai marker ở tầng type.

- Entity sở hữu `E`, dùng để theo dõi phạm vi bảng, tức là kiểm tra "bảng này đã có trong query chưa".
- Kiểu Rust `T`, dùng để chặn toán tử và để bind giá trị.

`Column` là `Copy`, nên dùng lại nhiều lần trong cùng một query không cần clone.

## 2.2. Các thuộc tính

### `#[oxider(table = "...")]` trên struct

Tên bảng dùng trong SQL.
Bỏ qua thì mặc định là tên struct viết thường:

```rust
#[derive(Entity)]      // không có #[oxider(...)]
struct Invoice { id: i64, amount: f64 }
// => TABLE = "invoice", FROM "invoice"
```

### `#[oxider(schema = "...")]` trên struct

Qualifier schema.
Schema chỉ xuất hiện ở mệnh đề `FROM`; tham chiếu cột vẫn dùng tên bảng trần, vì đó là cách mọi engine phân giải nguồn đã qualify:

```rust
#[derive(Entity)]
#[oxider(table = "audit_log", schema = "ops")]
struct AuditLog {
    id: i64,
    action: String,
}

AuditLog::query().select(AuditLog::action);
// SELECT "audit_log"."action" FROM "ops"."audit_log"
```

### `#[oxider(column = "...")]` trên field

Tên cột, khi nó khác tên field:

```rust
#[derive(Entity)]
#[oxider(table = "audit_log", schema = "ops")]
struct AuditLog {
    id: i64,
    #[oxider(column = "actor")]
    actor_name: String,
}
// AuditLog::actor_name  =>  "audit_log"."actor"
```

### `#[oxider(skip)]` trên field

Bỏ field ra khỏi metamodel, dành cho dữ liệu tính sau khi load chứ không phải cột:

```rust
#[derive(Entity)]
#[oxider(table = "people")]
struct Person {
    id: i64,
    name: String,
    #[oxider(skip)]
    display: String,
}
// Person::display không tồn tại; dùng tới nó là lỗi biên dịch.
```

## 2.3. Cột trùng từ khóa của Rust

SQL cho phép cột tên `type`, `match`, `ref`.
Rust bắt bạn viết chúng dưới dạng raw identifier, và macro tự bỏ tiền tố `r#` khi suy ra tên cột:

```rust
#[derive(Entity)]
#[oxider(table = "events")]
struct Event {
    id: i64,
    r#type: String,
    r#match: Option<String>,
}

Event::query().filter(Event::r#type.eq("signup"));
// SELECT * FROM "events" WHERE "events"."type" = $1
```

Với tên cột không thể thành identifier Rust bằng mọi cách, ví dụ `total-count` hay `2fa enabled`, hãy đặt tên field hợp lệ rồi khai báo tên thật bằng `#[oxider(column = "...")]`.
Đó cũng chính là thứ [bộ sinh entity](./13-codegen.md) tự làm cho bạn.

## 2.4. Ánh xạ kiểu và cột nullable

Kiểu cột `T` chính là kiểu field.
Field `Option<T>` đánh dấu cột nullable, và cột mang kiểu bên trong `T`:

| Field Rust | `Column` sinh ra | `.nullable` |
|---|---|---|
| `id: i64` | `Column<Self, i64>` | `false` |
| `name: String` | `Column<Self, String>` | `false` |
| `email: Option<String>` | `Column<Self, String>` | `true` |
| `score: Option<f64>` | `Column<Self, f64>` | `true` |

Đây là một quyết định thiết kế có chủ đích, không phải thiếu sót.

Nullability là cờ runtime, không nằm trong kiểu.
Nếu `Option<String>` sinh ra `Column<Self, Option<String>>` thì mọi toán tử chuỗi sẽ biến mất khỏi đúng những cột cần chúng nhất, và mọi vị trí nhận kiểu tự do trở nên nhập nhằng với trình biên dịch.
Hướng đó đã được thử và phải bỏ.
Vì vậy hôm nay `User::email.starts_with("a")` hoạt động bình thường, còn `User::email.nullable` là `true` để codegen và tài liệu đọc được.

## 2.5. Kiểu được hỗ trợ sẵn

Bind giá trị hoạt động với kiểu nào implement `ToSqlValue`.
Sẵn có cho: `bool`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `f32`, `f64`, `String`, `Vec<u8>`, và `Option<T>` khi `T: ToSqlValue`.

Bật feature `chrono` (mặc định bật) thì có thêm `NaiveDate`, `NaiveTime`, `NaiveDateTime`, `DateTime<Utc>`.

Giá trị được thu về một biến thể `Value`:

```rust
pub enum Value {
    Bool(bool),
    Int(i64),        // mọi kiểu nguyên nới rộng về i64
    Real(f64),       // f32/f64 nới rộng về f64
    Text(String),
    Bytes(Vec<u8>),
    Date(String),
    Time(String),
    DateTime(String),
    Null,            // từ Option::None
}
```

Ba marker phụ quyết định toán tử nào khả dụng.

- `Orderable` cho `<`, `>`, `<=`, `>=`, `BETWEEN`, `ORDER BY`, `MIN`/`MAX`.
  Có cho mọi kiểu số, `String` và kiểu thời gian. Không có cho `bool` và `Vec<u8>`.
- `Numeric` cho số học và `SUM`/`AVG`/`STDDEV`/`VARIANCE`.
  Chỉ có cho kiểu số.
- `Temporal` cho các toán tử ngày giờ (`year()`, `add_days()`, `truncate_to_month()`, ...).

Vì vậy `User::age.gt(18)` hợp lệ, `User::active.gt(true)` không biên dịch được vì `bool` không `Orderable`, và `Order::status.sum()` không biên dịch được vì `String` không `Numeric`.

## 2.6. Nới rộng kiểu ở vị trí toán hạng

Mọi vị trí toán hạng nhận `impl IntoExpr<T>`, nên một method `eq` phục vụ cả "so với giá trị" lẫn "so với cột khác":

```rust
User::age.eq(18)               // so với giá trị
User::department_id.eq(Department::id)   // so với cột, dùng cho khóa join
```

Danh sách nới rộng tường minh, để `.eq(18)` trên cột `i64` không phải viết `18i64`:

| Viết | Cột kiểu |
|---|---|
| `&str` | `String` |
| `&String` | `String` |
| `i8`, `i16`, `i32`, `u8`, `u16`, `u32` | `i64` |
| `i8`, `i16`, `u8`, `u16` | `i32` |
| `f32` | `f64` |
| `Option<T>` | `T` (sinh `NULL` khi `None`) |

Danh sách này là tường minh chứ không phải một impl tổng quát `V: Into<T>`, vì impl tổng quát vi phạm coherence với impl dành cho `Column`.

## 2.7. Kiểu domain tùy biến

Dùng kiểu riêng làm kiểu cột bằng cách implement `ToSqlValue` và `SqlType`, cộng `Orderable`/`Numeric` nếu muốn nhóm toán tử tương ứng:

```rust
use oxider_query::prelude::*;
use oxider_query::Value;

#[derive(Clone, Copy)]
enum Status {
    Active,
    Suspended,
}

impl SqlType for Status {}

impl ToSqlValue for Status {
    fn to_sql_value(&self) -> Value {
        match self {
            Status::Active => Value::Text("active".into()),
            Status::Suspended => Value::Text("suspended".into()),
        }
    }
}

#[derive(Entity)]
#[oxider(table = "accounts")]
struct Account {
    id: i64,
    status: Status,
}

let q = Account::query().filter(Account::status.eq(Status::Active));
```

Nếu muốn cột `Status` nhận thêm `&str`, hãy tự viết một impl `IntoExpr<Status> for &str`.

## 2.8. Alias bảng

Alias cần cho self-join và cho việc tham chiếu hai bản sao của cùng một bảng.

`Table::alias` đặt alias cho nguồn, `Column::at` đặt alias cho tham chiếu cột:

```rust
User::query()
    .inner_join(
        User::table().alias("manager"),
        User::id.at("manager").eq(User::manager_id),
    )
    .select((User::name, User::name.at("manager")));
// SELECT "users"."name", "manager"."name" FROM "users"
// INNER JOIN "users" AS "manager" ON "manager"."id" = "users"."manager_id"
```

Điểm mấu chốt: `User::id.at("manager")` có kiểu `Column<Aliased<User>, i64>`, không phải `Column<User, i64>`.

`Aliased<E>` là một entity riêng ở tầng type.
Nhờ vậy hai bản sao của bảng `users` là hai thành viên khác nhau trong tập phạm vi, và trình biên dịch tìm được đúng một chứng cứ cho mỗi tham chiếu.
Nếu cả hai cùng mang marker `User` thì việc "chứng minh `User` có trong scope" trở nên nhập nhằng và mọi self-join sẽ không biên dịch được.

Muốn bắt đầu query từ một bảng đã đặt alias, dùng `query_as`:

```rust
User::query_as("u").select(User::name.at("u"));
// SELECT "u"."name" FROM "users" AS "u"
```

Giới hạn cần biết: mỗi entity chỉ có một alias trong một query.
Bản sao thứ ba của cùng bảng phải tham chiếu bằng [`col`](./04-operators.md), thứ không được kiểm tra phạm vi.
Đây là đánh đổi có ý thức: giữ kiểm tra phạm vi đúng cho trường hợp thường gặp, thay vì bỏ hẳn nó để phục vụ trường hợp hiếm.

## Bước tiếp theo

Sang [chương 3](./03-select-and-filtering.md) để dựng câu SELECT đầy đủ.

# 2. Định nghĩa entity và cột

Entity là điểm khởi đầu của mọi query.
Chương này mô tả đầy đủ `#[derive(Entity)]`: nó sinh ra gì, ánh xạ kiểu ra sao, và cách mở rộng cho kiểu domain riêng.

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

Derive `Entity` làm hai việc:

1. Impl trait `Entity` cho struct, cung cấp `User::query()`, `User::insert()`, `User::update()`, `User::delete()` và hằng `User::TABLE`.
2. Sinh một associated const `Column<User, T>` cho mỗi field, tên const trùng tên field.

Với ví dụ trên bạn có `User::id: Column<User, i64>`, `User::name: Column<User, String>`, `User::email: Column<User, String>` (nullable).

Mỗi `Column` mang hai marker ở tầng type:

- Entity sở hữu `E` - dùng để theo dõi phạm vi bảng (bảng đã join chưa) và khớp khóa join.
- Kiểu Rust `T` - dùng để chặn toán tử (chỉ so sánh với giá trị đổi được sang `T`) và để bind giá trị.

## 2.2. Thuộc tính `#[oxider(table = "...")]`

Thuộc tính này chỉ tên bảng dùng trong SQL.
Nếu bỏ qua, tên bảng mặc định là tên struct viết thường:

```rust
#[derive(Entity)]      // không có #[oxider(...)]
struct Product { id: i64 }
// => TABLE = "product"
```

Thuộc tính hợp lệ duy nhất hiện nay là `table`.
Chưa hỗ trợ đổi tên từng cột: tên cột SQL luôn bằng tên field verbatim.
Nếu cột trong DB tên `created_at` thì field phải đặt tên `created_at`.

## 2.3. Ánh xạ kiểu và cột nullable

Kiểu cột `T` chính là kiểu field.
Field `Option<T>` đánh dấu cột nullable và cột mang kiểu bên trong `T`:

| Field Rust | `Column` sinh ra | Nullable |
|------------|------------------|----------|
| `id: i64` | `Column<Self, i64>` | không |
| `name: String` | `Column<Self, String>` | không |
| `email: Option<String>` | `Column<Self, String>` | có |
| `score: Option<f64>` | `Column<Self, f64>` | có |

Cờ nullable hiện lưu ở runtime trên `Column` (trường `.nullable`).
Type-level nullability (để outer join tự nâng cột non-null thành `Option`) là việc của giai đoạn sau, khi có tầng map row.
Nghĩa là hôm nay bạn vẫn so sánh `User::email` như một `String` bình thường; cờ nullable chưa ảnh hưởng tới kiểu trả về.

Chỉ struct có field đặt tên mới derive được.
Tuple struct và unit struct sẽ báo lỗi biên dịch.

## 2.4. Kiểu được hỗ trợ sẵn

Các toán tử so sánh và bind giá trị hoạt động với kiểu nào implement `ToSqlValue`.
Sẵn có cho: `i16`, `i32`, `i64`, `f32`, `f64`, `bool`, `String`, `&str`, và `Option<T>` khi `T: ToSqlValue`.

Giá trị được thu về một trong các biến thể `Value`:

```rust
pub enum Value {
    Bool(bool),
    Int(i64),   // i16/i32/i64 đều nới rộng về i64
    Real(f64),  // f32/f64 nới rộng về f64
    Text(String),
    Null,       // từ Option::None
}
```

Hai marker phụ quyết định toán tử nào khả dụng:

- `Orderable` - cho `<`, `>`, `<=`, `>=`, `ORDER BY`, và `MIN`/`MAX`.
  Có cho mọi kiểu số và `String`. Không có cho `bool`.
- `Numeric` - cho aggregate `SUM`/`AVG`.
  Chỉ có cho kiểu số.

Vì vậy `User::age.gt(18)` hợp lệ nhưng `User::active.gt(true)` không biên dịch được (`bool` không `Orderable`), còn `sum(User::name)` không biên dịch (`String` không `Numeric`).

## 2.5. Kiểu domain tùy biến

Bạn có thể dùng kiểu riêng làm kiểu cột, miễn là nó implement `ToSqlValue` (và `Orderable`/`Numeric` nếu muốn dùng toán tử tương ứng).

```rust
use oxider_query::prelude::*;
use oxider_query::Value;

#[derive(Clone, Copy)]
enum Status {
    Active,
    Suspended,
}

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

// Nhờ impl ToSqlValue, so sánh với Status hoạt động:
let q = Account::query().filter(Account::status.eq(Status::Active));
```

Toán tử so sánh nhận bất kỳ `V: Into<T>`, nên nếu cột kiểu `String` thì `.eq("active")` dùng được luôn (`&str: Into<String>`).
Nếu muốn cột `Status` cũng nhận `&str`, hãy impl `From<&str> for Status`.

## Bước tiếp theo

Sang [chương 3](./03-select-and-filtering.md) để dựng câu SELECT: projection, lọc, sắp xếp, phân trang và query động.

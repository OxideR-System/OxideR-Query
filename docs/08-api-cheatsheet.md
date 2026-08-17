# 8. Tra cứu nhanh API

Bảng tổng hợp mọi điểm vào chính.
Mọi thứ trong bảng đều có ở `oxider_query::prelude`.

## 8.1. Khởi tạo (trait `Entity`)

| Gọi | Trả về | Mục đích |
|-----|--------|----------|
| `E::query()` | `Select<Cons<E, Nil>>` | Bắt đầu SELECT |
| `E::insert()` | `Insert<E>` | Bắt đầu INSERT |
| `E::update()` | `Update<E>` | Bắt đầu UPDATE |
| `E::delete()` | `Delete<E>` | Bắt đầu DELETE |
| `E::TABLE` | `&'static str` | Tên bảng |

## 8.2. Toán tử trên `Column<E, T>`

| Method | Điều kiện | SQL |
|--------|-----------|-----|
| `eq(v)`, `ne(v)` | `T: ToSqlValue` | `=`, `<>` |
| `gt(v)`, `ge(v)`, `lt(v)`, `le(v)` | `T: Orderable` | `>`, `>=`, `<`, `<=` |
| `asc()`, `desc()` | `T: Orderable` | `ORDER BY ... ASC/DESC` |
| `like(p)`, `contains(s)`, `starts_with(p)` | `T = String` | `LIKE` |
| `eq_column(other)` | cột khác cùng `T` | `=` (khóa join) |
| `in_subquery(sub)`, `not_in_subquery(sub)` | `Subquery<T>` | `IN` / `NOT IN` |

Mọi toán tử giá trị nhận `V: Into<T>`.

## 8.3. Mệnh đề của `Select<S>`

| Method | Ghi chú |
|--------|---------|
| `.select(item hoặc tuple 2..=6)` | Projection; mặc định là tất cả cột |
| `.filter(pred)` | WHERE; gọi nhiều lần nối bằng AND |
| `.filter_opt(Option<pred>)` | WHERE có điều kiện (query động) |
| `.join::<E2>(on)` | INNER JOIN, mở rộng phạm vi |
| `.left_join::<E2>(on)` | LEFT JOIN, mở rộng phạm vi |
| `.group_by(item hoặc tuple)` | GROUP BY |
| `.having(pred)` | HAVING |
| `.order_by(col.asc()/desc())` | ORDER BY; gọi nhiều lần để nhiều khóa |
| `.limit(u64)`, `.offset(u64)` | Phân trang |
| `.scalar(item)` | -> `Subquery<T>` cho IN |
| `.build()` | -> `SelectQuery` (AST) |
| `.render(&dialect)` | -> `Rendered { sql, params }` |

Mọi mệnh đề tham chiếu cột đòi bảng liên quan nằm trong phạm vi `S`.

## 8.4. Ghép predicate

| Method | SQL |
|--------|-----|
| `pred.and(other)` | `(a AND b)` |
| `pred.or(other)` | `(a OR b)` |
| `exists(select)` | `EXISTS (...)` |
| `not_exists(select)` | `NOT EXISTS (...)` |

## 8.5. Aggregate

| Hàm | Kiểu vào | Kiểu ra |
|-----|----------|---------|
| `count_all()` | - | `i64` |
| `count(col)` | mọi cột | `i64` |
| `sum(col)` | `Numeric` | `T` |
| `avg(col)` | `Numeric` | `f64` |
| `min(col)`, `max(col)` | `Orderable` | `T` |

Aggregate dùng trong `.select(...)` và `.having(...)`; có `eq/ne/gt/ge/lt/le` để dựng điều kiện HAVING.

## 8.6. Mutation

| Builder | Chuỗi method |
|---------|--------------|
| `Insert<E>` | `.value(col, v)*` `.render(&d)` |
| `Update<E>` | `.set(col, v)*` `.filter(pred)?` `.render(&d)` |
| `Delete<E>` | `.filter(pred)?` `.render(&d)` |

`render` của mutation nhận `&self` (không tiêu thụ builder).

## 8.7. Dialect và kết quả

| Kiểu | Vai trò |
|------|---------|
| `Postgres`, `MySql`, `Sqlite` | Ba dialect sẵn có |
| `Dialect` (trait) | `quote_ident`, `placeholder`; tự impl để thêm dialect |
| `Rendered { sql: String, params: Vec<Value> }` | Kết quả render |
| `Value` (`Bool/Int/Real/Text/Null`) | Giá trị param đã bind |

## 8.8. Thực thi (`oxider-query-exec`, feature `sqlite`)

Một handle `Db` cho mọi backend (`SqliteDb = Db<Sqlite>` hôm nay). Tạo bằng `Db::connect(url)` hoặc `Db::new(pool)`.

| Method | Trả về |
|--------|--------|
| `db.execute(query)` | `u64` số dòng ảnh hưởng |
| `db.fetch_all(query)` | `Vec<O>` |
| `db.fetch_one(query)` | `O` |
| `db.fetch_optional(query)` | `Option<O>` |
| `db.pool()` | `&Pool` sqlx bên dưới |
| `db.begin()` | `Tx` (transaction thủ công) |
| `db.transaction(async \|tx\| ...)` | chạy closure trong transaction, commit nếu `Ok` / rollback nếu `Err` |

`query: Renderable` (`Select`/`Insert`/`Update`/`Delete`/`Rendered`) - truyền thẳng, không cần `.render(&...)`; `Db` tự render theo dialect của backend.
`O: sqlx::FromRow`. Thêm backend = impl trait `Backend` cho sqlx `Database` tương ứng.

Transaction `Tx` (từ `db.begin()`) có cùng `execute`/`fetch_all`/`fetch_one`/`fetch_optional` (nhận `&mut self`), kết thúc bằng `tx.commit()` hoặc `tx.rollback()`.
Hoặc `db.transaction(async |tx| ...)` cho phạm vi tự commit/rollback theo kết quả closure.

## 8.9. Codegen (`oxider-query-codegen`, feature `sqlite`)

| Hàm | Trả về |
|-----|--------|
| `generate_entities(&SqlitePool)` | `String` mã nguồn struct `#[derive(Entity)]` |

## 8.10. Marker kiểu

| Trait | Ý nghĩa | Có cho |
|-------|---------|--------|
| `ToSqlValue` | bind được thành `Value` | số, `bool`, `String`, `&str`, `Option<T>` |
| `Orderable` | so sánh thứ tự + ORDER BY + MIN/MAX | số, `String` |
| `Numeric` | SUM/AVG | chỉ số |
| `Renderable` | render được thành `Rendered` (lớp exec nhận vào) | `Select`, `SelectQuery`, `Insert`, `Update`, `Delete`, `Rendered` |

Impl các trait này cho kiểu domain riêng để dùng chúng làm kiểu cột.

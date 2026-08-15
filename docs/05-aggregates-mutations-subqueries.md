# 5. Aggregate, mutation và subquery

Chương này gộp ba nhóm tính năng còn lại của builder: hàm thống kê kèm GROUP BY/HAVING, các câu ghi dữ liệu, và subquery.

## 5.1. Aggregate và GROUP BY / HAVING

Các hàm aggregate nằm ở prelude:

| Hàm | Cột nhận vào | Kiểu kết quả | SQL |
|-----|--------------|--------------|-----|
| `count_all()` | không | `i64` | `COUNT(*)` |
| `count(col)` | mọi cột | `i64` | `COUNT(col)` |
| `sum(col)` | `T: Numeric` | `T` | `SUM(col)` |
| `avg(col)` | `T: Numeric` | `f64` | `AVG(col)` |
| `min(col)` | `T: Orderable` | `T` | `MIN(col)` |
| `max(col)` | `T: Orderable` | `T` | `MAX(col)` |

Aggregate dùng được trong projection (`select`) và trong `having`.
`sum`/`avg` chỉ nhận cột số nên `sum(Order::status)` (status là `String`) không biên dịch.

```rust
use oxider_query::prelude::*;

#[derive(Entity)]
#[oxider(table = "orders")]
struct Order { id: i64, customer_id: i64, status: String, amount: i64 }

let rows = Order::query()
    .select((Order::customer_id, count_all()))
    .filter(Order::status.eq("paid"))
    .group_by(Order::customer_id)
    .having(count_all().gt(5))
    .render(&Postgres);

// SELECT "orders"."customer_id", COUNT(*) FROM "orders"
// WHERE ("orders"."status" = $1)
// GROUP BY "orders"."customer_id" HAVING (COUNT(*) > $2)
```

`group_by` nhận một cột hoặc tuple cột giống `select`.
`having` nhận một predicate; aggregate có sẵn các toán tử so sánh (`eq`, `ne`, `gt`, `ge`, `lt`, `le`) để dựng điều kiện HAVING, ví dụ `sum(Order::amount).gt(1000)`.

## 5.2. INSERT

`Entity::insert()` mở một câu INSERT.
Mỗi `value(column, v)` gán một cột; cột phải thuộc đúng entity đang insert.

```rust
User::insert()
    .value(User::name, "Alice")
    .value(User::age, 30)
    .render(&Postgres);
// INSERT INTO "users" ("name", "age") VALUES ($1, $2)
// params: ["Alice", 30]
```

Gán cột của entity khác là lỗi biên dịch (mutation chỉ một bảng).

## 5.3. UPDATE

`Entity::update()` mở câu UPDATE.
`set(column, v)` thêm phép gán, `filter(pred)` thêm điều kiện WHERE.

```rust
User::update()
    .set(User::name, "Bob")
    .filter(User::id.eq(7))
    .render(&Postgres);
// UPDATE "users" SET "name" = $1 WHERE ("users"."id" = $2)
// params: ["Bob", 7]
```

Param của SET đứng trước param của WHERE, khớp thứ tự trong SQL.
Có thể gọi `set` nhiều lần cho nhiều cột.

Cảnh báo: `update()` không bắt buộc có `filter`.
Không gọi `filter` sẽ sinh UPDATE toàn bảng, đúng như SQL thuần; hãy cẩn thận.

## 5.4. DELETE

`Entity::delete()` mở câu DELETE, `filter(pred)` thêm WHERE:

```rust
User::delete()
    .filter(User::active.eq(false))
    .render(&Postgres);
// DELETE FROM "users" WHERE ("users"."active" = $1)
```

Tương tự UPDATE, DELETE không filter sẽ xóa toàn bảng.

## 5.5. Subquery IN / NOT IN

`scalar(item)` trên một `Select` biến nó thành `Subquery<T>` chọn đúng một cột kiểu `T`.
`Column::in_subquery` / `not_in_subquery` nhận subquery và yêu cầu cột outer cùng kiểu `T`, nên lệch kiểu là lỗi biên dịch.

```rust
#[derive(Entity)]
#[oxider(table = "departments")]
struct Department { id: i64, name: String, active: bool }

let active = Department::query()
    .filter(Department::active.eq(true))
    .scalar(Department::id);

User::query()
    .filter(User::department_id.in_subquery(active))
    .render(&Postgres);

// SELECT ... FROM "users"
// WHERE ("users"."department_id" IN (
//   SELECT "departments"."id" FROM "departments" WHERE ("departments"."active" = $1)))
```

Vì `Department::id` là `i64` và `User::department_id` là `i64` nên hợp lệ.
Nếu `scalar(Department::name)` (String) thì `in_subquery` với cột `i64` không biên dịch.

## 5.6. EXISTS / NOT EXISTS

`exists(select)` và `not_exists(select)` nhận một `Select` bất kỳ và trả về một `Predicate` luôn nằm trong phạm vi (dùng được ở mọi nơi chấp nhận predicate):

```rust
let has_paid = Order::query().filter(Order::status.eq("paid"));

User::query()
    .filter(exists(has_paid))
    .render(&Postgres);
// WHERE EXISTS (SELECT ... FROM "orders" WHERE ("orders"."status" = $1))
```

Subquery hiện là uncorrelated (không tham chiếu cột của query ngoài).
Correlated subquery là hạng mục tương lai.

## Bước tiếp theo

Sang [chương 6](./06-dialects-execution-codegen.md) để render đa dialect, thực thi query qua sqlx và sinh entity từ schema.

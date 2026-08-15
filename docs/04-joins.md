# 4. JOIN và phạm vi bảng ở tầng type

OxideR-Query theo dõi tập bảng đang trong phạm vi query ngay ở tầng type.
Bạn chỉ tham chiếu được cột của bảng đã có mặt (qua FROM hoặc JOIN); tham chiếu bảng chưa join là lỗi biên dịch.

## 4.1. INNER JOIN

`join::<E2>(on)` thêm một INNER JOIN tới entity `E2`.
Điều kiện join thường là `eq_column` giữa hai cột cùng kiểu:

```rust
use oxider_query::prelude::*;

#[derive(Entity)]
#[oxider(table = "users")]
struct User { id: i64, name: String, department_id: i64 }

#[derive(Entity)]
#[oxider(table = "departments")]
struct Department { id: i64, name: String }

let rows = User::query()
    .join::<Department>(User::department_id.eq_column(Department::id))
    .select((User::name, Department::name))
    .filter(Department::name.eq("AI"))
    .render(&Postgres);

// SELECT "users"."name", "departments"."name" FROM "users"
// INNER JOIN "departments" ON ("users"."department_id" = "departments"."id")
// WHERE ("departments"."name" = $1)
```

Chỉ cần chỉ `E2` trong turbofish (`join::<Department>`); kiểu của điều kiện `on` được suy ra.

## 4.2. LEFT JOIN

`left_join::<E2>(on)` giống hệt nhưng sinh `LEFT JOIN`:

```rust
User::query()
    .left_join::<Department>(User::department_id.eq_column(Department::id))
    .select((User::name, Department::name));
// FROM "users" LEFT JOIN "departments" ON (...)
```

Lưu ý: hôm nay cột của bảng bên phải LEFT JOIN vẫn giữ nguyên kiểu (không tự thành `Option`).
Type-level nullability cho outer join được hoãn tới khi có tầng map row->struct (lúc đó cột bên phải mới có ý nghĩa hiện ra là `Option<_>`).

## 4.3. Khóa join type-safe

`eq_column` yêu cầu hai cột cùng kiểu Rust `T`.
Ghép khóa lệch kiểu là lỗi biên dịch, bắt sớm lỗi thiết kế schema:

```rust
// department_id: i64, Department::id: i64  -> hợp lệ
User::department_id.eq_column(Department::id);

// Nếu Department::id là String thì dòng trên KHÔNG biên dịch:
// error: mismatched types, expected Column<_, i64>, found Column<_, String>
```

Kết quả `eq_column` tham chiếu cả hai entity, nên sau khi join bạn dùng được cột của cả hai.

## 4.4. Phạm vi bảng: chỉ ref cột đã có trong scope

`User::query()` khởi tạo `Select<Cons<User, Nil>>`: phạm vi ban đầu chỉ có `User`.
Mỗi `join`/`left_join` mở rộng phạm vi: `join::<Department>` cho `Select<Cons<Department, Cons<User, Nil>>>`.

Các mệnh đề `filter`, `select`, `order_by`, `group_by`, `having` đều đòi mọi bảng chúng tham chiếu phải nằm trong phạm vi.
Tham chiếu bảng chưa join là lỗi biên dịch:

```rust
// Chưa join Department -> lỗi:
User::query().filter(Department::name.eq("AI"));
// error: the trait bound `Nil: Contains<Department, _>` is not satisfied
```

Đây là cơ chế "column-belongs-to-a-joined-table" thực thi hoàn toàn ở compile time, không tốn gì lúc chạy.

## 4.5. Join nhiều bảng

Nối nhiều `join` để mở rộng phạm vi dần:

```rust
#[derive(Entity)]
#[oxider(table = "companies")]
struct Company { id: i64, name: String }

// giả sử Department có thêm company_id: i64
User::query()
    .join::<Department>(User::department_id.eq_column(Department::id))
    .join::<Company>(Department::company_id.eq_column(Company::id))
    .select((User::name, Department::name, Company::name))
    .filter(Company::name.eq("OxideR"));
// FROM "users"
// INNER JOIN "departments" ON ("users"."department_id" = "departments"."id")
// INNER JOIN "companies"   ON ("departments"."company_id" = "companies"."id")
// WHERE ("companies"."name" = $1)
```

Điều kiện join của bước sau có thể tham chiếu bảng đã join ở bước trước (`Department::company_id`), vì phạm vi tích lũy qua từng `join`.

## Bước tiếp theo

Sang [chương 5](./05-aggregates-mutations-subqueries.md) cho aggregate, GROUP BY/HAVING, INSERT/UPDATE/DELETE và subquery.

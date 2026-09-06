---
id: set-operations-and-ctes
title: 9. Set operation và CTE
sidebar_position: 9
---

# 9. Set operation và CTE

## 9.1. UNION, INTERSECT, EXCEPT

```rust
User::query()
    .select(User::name)
    .union(Department::query().select(Department::name));
```

```sql
-- PostgreSQL và MySQL: nhánh được bọc ngoặc
SELECT "users"."name" FROM "users"
UNION (SELECT "departments"."name" FROM "departments")

-- SQLite: từ chối nhánh bọc ngoặc, nên nhận dạng trần
SELECT "users"."name" FROM "users"
UNION SELECT "departments"."name" FROM "departments"
```

Bọc ngoặc hay không do `Caps::wrap_set_op_branches` quyết định, chứ không phải do query.

Sáu phương thức: `union`, `union_all`, `intersect`, `intersect_all`, `except`, `except_all`.

MySQL không có `INTERSECT` lẫn `EXCEPT`, và cả ba engine đều không hỗ trợ đủ dạng `ALL`.
Các trường hợp đó bị từ chối lúc render:

```rust
let q = User::query().select(User::name)
    .intersect(Department::query().select(Department::name));

q.to_sql(&Postgres).unwrap();
q.to_sql(&MySql).unwrap_err();   // UnsupportedFeature { feature: "INTERSECT", .. }
```

```rust
let q = User::query().select(User::name)
    .intersect_all(Department::query().select(Department::name));

q.to_sql(&Sqlite).unwrap_err();  // feature: "ALL form of a set operation"
```

## 9.2. ORDER BY và LIMIT của cả phép hợp

Mệnh đề thêm sau phép hợp áp dụng cho toàn bộ kết quả, không phải cho nhánh cuối:

```rust
User::query()
    .select(User::name)
    .union(Department::query().select(Department::name))
    .order_by(User::name.asc())
    .limit(10);
```

```sql
SELECT "users"."name" FROM "users"
UNION (SELECT "departments"."name" FROM "departments")
ORDER BY "users"."name" ASC LIMIT 10
```

## 9.3. CTE

`with(name, query)` khai báo một CTE trước query dùng nó.

CTE không có entity mô tả, nên cột của nó đọc bằng `col`, và nó tham gia `FROM` như một nguồn có tên:

```rust
let recent = Post::query()
    .filter(Post::views.gt(100))
    .select((Post::user_id, Post::title));

oxider_query::select_from_name("recent")
    .with("recent", recent)
    .select((col::<i64>("recent", "user_id"), col::<String>("recent", "title")))
    .order_by(col::<String>("recent", "title").asc());
```

```sql
WITH "recent" AS (
  SELECT "posts"."user_id", "posts"."title" FROM "posts" WHERE "posts"."views" > $1
)
SELECT "recent"."user_id", "recent"."title" FROM "recent"
ORDER BY "recent"."title" ASC
```

Một CTE cũng join được vào query bình thường, dưới dạng derived table:

```rust
let busy = Post::query()
    .select((Post::user_id, count_all().alias("post_count")))
    .group_by(Post::user_id)
    .having(count_all().gt(10));

User::query()
    .with("busy", busy)
    .join_query(
        oxider_query::select_from_name("busy").select(col::<i64>("busy", "user_id")),
        "b",
        User::id.eq(col::<i64>("b", "user_id")),
    )
    .select(User::name);
```

## 9.4. CTE tự đặt tên cột

```rust
let totals = Order::query()
    .select((Order::user_id, Order::total.sum()))
    .group_by(Order::user_id);

oxider_query::select_from_name("totals")
    .with_columns("totals", ["user_id", "spent"], totals)
    .select(col::<f64>("totals", "spent"));
```

```sql
WITH "totals" ("user_id", "spent") AS (
  SELECT "orders"."user_id", SUM("orders"."total") FROM "orders" GROUP BY "orders"."user_id"
)
SELECT "totals"."spent" FROM "totals"
```

## 9.5. CTE đệ quy

`recursive()` đánh dấu mệnh đề `WITH` là đệ quy.
Thân CTE là một phép hợp: nhánh neo, rồi nhánh đệ quy tham chiếu lại chính CTE.

```rust
// Nhánh neo: những người không có quản lý.
let anchor = User::query()
    .filter(User::manager_id.is_null())
    .select((User::id, User::name));

// Nhánh đệ quy: join ngược lại CTE bằng tên.
let step = User::query()
    .join_name_as("chain", "c", User::manager_id.eq(col::<i64>("c", "id")))
    .select((User::id, User::name));

oxider_query::select_from_name("chain")
    .with("chain", anchor.union_all(step))
    .recursive()
    .select(col::<String>("chain", "name"));
```

```sql
WITH RECURSIVE "chain" AS (
  SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."manager_id" IS NULL
  UNION ALL
  (SELECT "users"."id", "users"."name" FROM "users"
   INNER JOIN "chain" AS "c" ON "users"."manager_id" = "c"."id")
)
SELECT "chain"."name" FROM "chain"
```

Chi tiết dễ sai: nhánh đệ quy phải dùng `join_name_as`, không phải `join_query`.

`join_query` bọc nguồn vào một derived table, và một CTE đệ quy chỉ được tham chiếu chính nó **trực tiếp** trong `FROM`.
Bọc nó lại thì database báo `circular reference: chain` và query không chạy.
Đây là lỗi chỉ lộ ra khi chạy trên database thật, nên nó có một test end-to-end riêng chạy trên SQLite.

## Bước tiếp theo

[Chương 10](./10-dml.md) ghi dữ liệu thay vì đọc.

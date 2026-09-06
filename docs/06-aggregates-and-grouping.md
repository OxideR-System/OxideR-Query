---
id: aggregates-and-grouping
title: 6. Aggregate và GROUP BY
sidebar_position: 6
---

# 6. Aggregate và GROUP BY

## 6.1. Các hàm aggregate

| Hàm | Bound | Kiểu trả về |
|---|---|---|
| `count_all()` | không cần cột | `i64` |
| `x.count()` | `T: SqlType` | `i64` |
| `x.count_distinct()` | `T: SqlType` | `i64` |
| `x.min()`, `x.max()` | `T: Orderable` | `T` |
| `x.sum()` | `T: Numeric` | `T` |
| `x.avg()` | `T: Numeric` | `f64` |
| `x.std_dev()`, `x.std_dev_pop()` | `T: Numeric` | `f64` |
| `x.variance()`, `x.var_pop()` | `T: Numeric` | `f64` |
| `bool_and(x)`, `bool_or(x)` | `T = bool` | `bool` |
| `group_concat(x, sep)` | `T = String` | `String` |

```rust
Order::query()
    .select((Order::total.sum(), Order::total.avg(), Order::total.max()))
    .filter(Order::status.eq("paid"));
// SELECT SUM("orders"."total"), AVG("orders"."total"), MAX("orders"."total")
// FROM "orders" WHERE "orders"."status" = $1
```

`Order::status.sum()` không biên dịch được, vì `String` không phải `Numeric`.

## 6.2. GROUP BY và HAVING

```rust
Post::query()
    .select((Post::user_id, count_all()))
    .group_by(Post::user_id)
    .having(count_all().gt(5))
    .order_by(Post::user_id.asc());
```

```sql
SELECT "posts"."user_id", COUNT(*) FROM "posts"
GROUP BY "posts"."user_id"
HAVING COUNT(*) > $1
ORDER BY "posts"."user_id" ASC
```

`group_by` nhận một biểu thức hoặc một tuple:

```rust
Order::query()
    .select((Order::user_id, Order::status, count_all()))
    .group_by((Order::user_id, Order::status));
// GROUP BY "orders"."user_id", "orders"."status"
```

`having` chịu cùng kiểm tra phạm vi như `filter`.

## 6.3. DISTINCT bên trong aggregate

```rust
Order::query().select(Order::user_id.count_distinct());
// SELECT COUNT(DISTINCT "orders"."user_id") FROM "orders"
```

Mọi aggregate đều có `.distinct()`; `count_distinct()` chỉ là tên gọi quen thuộc cho trường hợp hay dùng nhất.

## 6.4. Aggregate có điều kiện

`filter_where` giới hạn dòng nào được tính, mà không cần tách thành query riêng:

```rust
let paid = count_all().filter_where(Order::status.eq("paid"));
Order::query().select((Order::user_id, paid)).group_by(Order::user_id);
```

```sql
-- PostgreSQL và SQLite: mệnh đề FILTER native
SELECT "orders"."user_id", COUNT(*) FILTER (WHERE "orders"."status" = $1)
FROM "orders" GROUP BY "orders"."user_id"

-- MySQL: giả lập bằng CASE
SELECT `orders`.`user_id`, COUNT(CASE WHEN `orders`.`status` = ? THEN 1 END)
FROM `orders` GROUP BY `orders`.`user_id`
```

Bản giả lập đếm đúng những dòng mà bản native đếm, vì `COUNT` bỏ qua `NULL` mà `CASE` sinh ra cho dòng không khớp.
Điều này được kiểm chứng bằng test chạy trên database thật, so kết quả của hai đường với nhau, chứ không chỉ so chuỗi SQL.

Với aggregate có giá trị, biểu thức được aggregate mới là thứ bị bọc:

```rust
Order::total.sum().filter_where(Order::status.eq("paid"));
// MySQL:
// SUM(CASE WHEN `orders`.`status` = ? THEN `orders`.`total` END)
```

## 6.5. Nối chuỗi theo nhóm

Tên và hình dạng khác nhau ở cả ba engine:

```rust
Post::query().select(group_concat(Post::title, ", ")).group_by(Post::user_id);
```

```sql
-- PostgreSQL
STRING_AGG("posts"."title", $1)
-- MySQL
GROUP_CONCAT(`posts`.`title` SEPARATOR ?)
-- SQLite
GROUP_CONCAT("posts"."title", ?)
```

Sắp xếp bên trong lời gọi bằng `.order_by(...)`:

```rust
group_concat(Post::title, ", ").order_by(Post::title.asc());
// STRING_AGG("posts"."title" ORDER BY "posts"."title" ASC, $1)
```

## 6.6. Aggregate boolean

```rust
User::query().select(bool_and(User::active));
```

```sql
-- PostgreSQL
BOOL_AND("users"."active")
-- MySQL: giả lập, vì không có BOOL_AND
(MIN(`users`.`active`) <> 0)
```

## 6.7. Khi engine không có hàm, query bị từ chối

Không phải thiếu sót nào cũng giả lập được đúng.
Các aggregate thống kê không tồn tại trên SQLite, và giả lập chúng bằng biểu thức số học sẽ cho kết quả sai lệch ở dữ liệu thực.
Vì vậy chúng bị từ chối:

```rust
let query = Department::query().select(Department::budget.std_dev());

query.to_sql(&Postgres).unwrap();   // STDDEV("departments"."budget")
query.to_sql(&MySql).unwrap();      // STDDEV_SAMP(`departments`.`budget`)
query.to_sql(&Sqlite).unwrap_err(); // UnsupportedOperator { operator: StdDev, .. }
```

Nguyên tắc chung: giả lập khi kết quả giống hệt, từ chối khi không.
Một câu SQL "gần đúng" chạy được nhưng trả sai số liệu khó phát hiện hơn nhiều so với một lỗi ngay lúc render.

## 6.8. Aggregate qua join

```rust
User::query()
    .left_join(Order::table(), Order::user_id.eq(User::id))
    .select((User::name, Order::total.sum().alias("lifetime_value")))
    .group_by(User::name)
    .having(Order::total.sum().gt(1000.0))
    .order_by(User::name.asc());
```

```sql
SELECT "users"."name", SUM("orders"."total") AS "lifetime_value"
FROM "users" LEFT JOIN "orders" ON "orders"."user_id" = "users"."id"
GROUP BY "users"."name"
HAVING SUM("orders"."total") > $1
ORDER BY "users"."name" ASC
```

## Bước tiếp theo

[Chương 7](./07-window-functions.md) tính toán theo cửa sổ mà không gộp dòng lại.

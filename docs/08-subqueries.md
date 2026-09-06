---
id: subqueries
title: 8. Subquery
sidebar_position: 8
---

# 8. Subquery

Subquery tương quan là phần khó nhất của một query builder type-safe, và là lý do `Select` có hai tham số kiểu thay vì một.

## 8.1. Hai tham số kiểu của `Select`

```rust
Select<S, F = Nil>
```

- `S` là mọi thứ đang trong phạm vi của query này.
- `F` là các entity **tự do**, tức là những entity thuộc về query bên ngoài mà query này tham chiếu tới.

Với một query độc lập thì `F = Nil`.
`correlate::<E>()` thêm `E` vào cả hai: nó vào scope để bạn viết được điều kiện, và vào `F` để đánh dấu rằng biểu thức kết quả còn phụ thuộc vào bên ngoài.

Khi biến query thành biểu thức, chỉ `F` được giữ lại:

```rust
fn scalar(...) -> Subquery<F, T>
fn exists(query: Select<S, F>) -> Predicate<F>
```

Nhờ vậy, nhúng một subquery tương quan vào chỗ mà bảng ngoài không có trong phạm vi là lỗi biên dịch.

## 8.2. IN và NOT IN

`scalar(expr)` biến một query thành subquery trả về một cột:

```rust
let busy_authors = Post::query().filter(Post::views.gt(1000)).scalar(Post::user_id);
User::query().filter(User::id.in_subquery(busy_authors));
```

```sql
SELECT * FROM "users"
WHERE "users"."id" IN (SELECT "posts"."user_id" FROM "posts" WHERE "posts"."views" > $1)
```

`not_in_subquery` là dạng phủ định.

Kiểu được kiểm tra: `User::id` là `i64`, nên subquery phải chọn một cột `i64`.
Chọn `Post::title` ở đó là lỗi biên dịch.

## 8.3. EXISTS

`exists` bỏ qua projection và chỉ hỏi có dòng nào không:

```rust
let has_posts = Post::query()
    .correlate::<User>()
    .filter(Post::user_id.eq(User::id));

User::query().filter(exists(has_posts));
```

```sql
SELECT * FROM "users"
WHERE EXISTS (SELECT * FROM "posts" WHERE "posts"."user_id" = "users"."id")
```

`not_exists` tìm những dòng không có bản khớp:

```rust
let has_orders = Order::query()
    .correlate::<User>()
    .filter(Order::user_id.eq(User::id));

User::query().filter(not_exists(has_orders)).select(User::name);
// SELECT "users"."name" FROM "users"
// WHERE NOT EXISTS (SELECT * FROM "orders" WHERE "orders"."user_id" = "users"."id")
```

Bỏ `correlate::<User>()` đi thì `Order::user_id.eq(User::id)` không biên dịch, vì `User` không có trong phạm vi của query con.

## 8.4. Subquery vô hướng trong projection

```rust
let post_count = Post::query()
    .correlate::<User>()
    .filter(Post::user_id.eq(User::id))
    .scalar(count_all());

User::query().select((User::name, post_count.alias("posts")));
```

```sql
SELECT "users"."name",
       (SELECT COUNT(*) FROM "posts" WHERE "posts"."user_id" = "users"."id") AS "posts"
FROM "users"
```

Subquery không tương quan cũng dùng được ở mọi vị trí biểu thức:

```rust
let average = Order::query().scalar(Order::total.avg());
Order::query().filter(Order::total.gt(average)).select(Order::id);
// SELECT "orders"."id" FROM "orders"
// WHERE "orders"."total" > (SELECT AVG("orders"."total") FROM "orders")
```

## 8.5. ANY và ALL

```rust
let cheap = OrderItem::query()
    .filter(OrderItem::quantity.eq(1))
    .scalar(OrderItem::price);

OrderItem::query().filter(OrderItem::price.gt(cheap.any()));
// WHERE "order_items"."price" > ANY (SELECT ...)

OrderItem::query().filter(OrderItem::price.ge(cheap.all()));
// WHERE "order_items"."price" >= ALL (SELECT ...)
```

## 8.6. Tương quan nhiều tầng

Một subquery tương quan tới bảng nào thì bảng đó phải có mặt ở nơi nó được dùng, dù cách bao nhiêu tầng:

```rust
let item_total = OrderItem::query()
    .correlate::<Order>()
    .filter(OrderItem::order_id.eq(Order::id))
    .scalar(OrderItem::price.sum());

User::query()
    .inner_join(Order::table(), Order::user_id.eq(User::id))
    .select((User::name, item_total.alias("items_total")));
```

```sql
SELECT "users"."name",
       (SELECT SUM("order_items"."price") FROM "order_items"
        WHERE "order_items"."order_id" = "orders"."id") AS "items_total"
FROM "users" INNER JOIN "orders" ON "orders"."user_id" = "users"."id"
```

`item_total` mang `F = Cons<Order, Nil>`.
Dùng nó trong một query không có `Order` là lỗi biên dịch.

## 8.7. Đánh số placeholder qua nhiều tầng

Placeholder đếm liên tục theo thứ tự xuất hiện trong chuỗi SQL cuối cùng, kể cả khi đi xuyên qua subquery:

```rust
let recent = Post::query().filter(Post::views.gt(10)).scalar(Post::user_id);

User::query()
    .filter(User::active.eq(true))
    .filter(User::id.in_subquery(recent))
    .filter(User::age.ge(21));
```

```sql
SELECT * FROM "users"
WHERE "users"."active" = $1
  AND "users"."id" IN (SELECT "posts"."user_id" FROM "posts" WHERE "posts"."views" > $2)
  AND "users"."age" >= $3
```

Danh sách tham số là `[true, 10, 21]`, đúng thứ tự đó.

## Bước tiếp theo

[Chương 9](./09-set-operations-and-ctes.md) ghép nhiều query lại với nhau.

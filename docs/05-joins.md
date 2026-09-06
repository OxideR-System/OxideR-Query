---
id: joins
title: 5. JOIN
sidebar_position: 5
---

# 5. JOIN

Đây là chỗ hệ thống kiểu làm việc nhiều nhất.
Mỗi join mở rộng tập bảng đang có trong query, và mọi tham chiếu cột sau đó phải nằm trong tập ấy.

## 5.1. Join cơ bản

`inner_join` nhận một `Table<E2>` và một điều kiện.
Bảng vừa join đã có mặt trong phạm vi của chính điều kiện đó:

```rust
User::query()
    .inner_join(Post::table(), Post::user_id.eq(User::id))
    .select((User::name, Post::title));
```

```sql
SELECT "users"."name", "posts"."title" FROM "users"
INNER JOIN "posts" ON "posts"."user_id" = "users"."id"
```

Có `inner_join`, `left_join`, `right_join`, `full_join`, và `cross_join` (không có điều kiện).

`RIGHT JOIN` không có trên SQLite, `FULL JOIN` không có trên MySQL lẫn SQLite; các dialect đó từ chối query thay vì sinh SQL sai.

## 5.2. Nối nhiều bảng

Mỗi join mở rộng phạm vi cho join kế tiếp:

```rust
User::query()
    .inner_join(Order::table(), Order::user_id.eq(User::id))
    .inner_join(OrderItem::table(), OrderItem::order_id.eq(Order::id))
    .filter(User::active.eq(true).and(OrderItem::quantity.gt(1)))
    .select((User::name, OrderItem::product, OrderItem::quantity));
```

```sql
SELECT "users"."name", "order_items"."product", "order_items"."quantity"
FROM "users"
INNER JOIN "orders" ON "orders"."user_id" = "users"."id"
INNER JOIN "order_items" ON "order_items"."order_id" = "orders"."id"
WHERE "users"."active" = $1 AND "order_items"."quantity" > $2
```

Thử tham chiếu `OrderItem` trước khi join nó là lỗi biên dịch, không phải lỗi runtime.

## 5.3. Điều kiện join phức hợp

Điều kiện là một `Predicate` bình thường, nên nó ghép được như mọi điều kiện khác:

```rust
User::query().inner_join(
    Post::table(),
    Post::user_id.eq(User::id).and(Post::views.gt(100)),
);
// INNER JOIN "posts" ON "posts"."user_id" = "users"."id" AND "posts"."views" > $1
```

## 5.4. Self-join

Hai bản sao của cùng một bảng cần alias để phân biệt.
`Table::alias` đặt alias cho nguồn, `Column::at` cho tham chiếu cột:

```rust
User::query()
    .inner_join(
        User::table().alias("manager"),
        User::id.at("manager").eq(User::manager_id),
    )
    .select((User::name, User::name.at("manager")));
```

```sql
SELECT "users"."name", "manager"."name" FROM "users"
INNER JOIN "users" AS "manager" ON "manager"."id" = "users"."manager_id"
```

`User::id.at("manager")` có kiểu `Column<Aliased<User>, i64>`.
`Aliased<User>` là một entity riêng ở tầng type, nên trình biên dịch phân biệt được hai bản sao và tìm được đúng một chứng cứ phạm vi cho mỗi tham chiếu.
Xem [mục 2.8](./02-entities-and-columns.md) để hiểu vì sao điều đó là bắt buộc.

## 5.5. Derived table

`join_query` join một query con dưới một alias.
Các cột của nó đọc bằng `col`, vì không có entity nào mô tả chúng:

```rust
let totals = Order::query()
    .select((Order::user_id, Order::total.sum().alias("spent")))
    .group_by(Order::user_id);

User::query()
    .join_query(totals, "totals", col::<i64>("totals", "user_id").eq(User::id))
    .select((User::name, col::<f64>("totals", "spent")));
```

```sql
SELECT "users"."name", "totals"."spent" FROM "users"
INNER JOIN (
  SELECT "orders"."user_id", SUM("orders"."total") AS "spent"
  FROM "orders" GROUP BY "orders"."user_id"
) AS "totals" ON "totals"."user_id" = "users"."id"
```

## 5.6. Join một nguồn chỉ có tên

`join_name` và `join_name_as` join một nguồn theo tên trần, không bọc nó vào subquery.

Đây là cách bắt buộc để tham chiếu một CTE.
Một CTE đệ quy chỉ được tham chiếu chính nó trực tiếp trong `FROM`; bọc nó vào derived table sẽ thành tham chiếu vòng và database từ chối:

```rust
User::query()
    .join_name_as("chain", "c", User::manager_id.eq(col::<i64>("c", "id")))
    .select((User::id, User::name));
```

```sql
SELECT "users"."id", "users"."name" FROM "users"
INNER JOIN "chain" AS "c" ON "users"."manager_id" = "c"."id"
```

Ví dụ đầy đủ của CTE đệ quy ở [chương 9](./09-set-operations-and-ctes.md).

## 5.7. Comma join

`and_from` thêm một mục nữa vào danh sách `FROM`, tương đương join theo dấu phẩy:

```rust
User::query()
    .and_from(Department::table())
    .filter(User::department_id.eq(Department::id))
    .select((User::name, Department::name));
```

```sql
SELECT "users"."name", "departments"."name"
FROM "users", "departments"
WHERE "users"."department_id" = "departments"."id"
```

Có thêm `and_from_query` cho một query con, và `and_from_name` cho một nguồn chỉ có tên.

## 5.8. Kiểm tra phạm vi hoạt động ra sao

Tập bảng trong scope là một danh sách ở tầng type, ghép bằng `Cons`, kết thúc bằng `Nil`.
`Select<S, F>` mang `S` là tập ấy.

Mỗi lần join thêm một entity vào đầu danh sách: `Select<Cons<Post, Cons<User, Nil>>>`.

Khi bạn đưa một `Predicate<S2>` vào `filter`, ràng buộc `S: ContainsAll<S2, I>` yêu cầu mọi entity trong `S2` phải có mặt trong `S`.
Chứng cứ `I` là danh sách chỉ số vị trí, được trình biên dịch tự suy.

Hệ quả thực tế:

- Quên join một bảng thì lỗi biên dịch, thông báo dạng `the trait bound Nil: Contains<Department, _> is not satisfied`.
- Thứ tự join không quan trọng, chỉ cần bảng có mặt trước khi được nhắc tới.
- Không có chi phí runtime nào: mọi thứ bị xóa sạch sau khi biên dịch.

Giới hạn: mỗi entity chỉ được có một alias trong một query, vì `Aliased<E>` không mang tên alias vào kiểu.
Bản sao thứ ba của cùng bảng phải dùng `col`, và mất kiểm tra phạm vi cho riêng nó.

## Bước tiếp theo

[Chương 6](./06-aggregates-and-grouping.md) gộp dòng lại thành nhóm.

---
id: window-functions
title: 7. Window function
sidebar_position: 7
---

# 7. Window function

Window function tính toán trên một nhóm dòng liên quan mà không gộp chúng lại thành một dòng.

Cấu trúc luôn giống nhau: một aggregate hoặc một hàm xếp hạng, rồi `.over(window)`.

## 7.1. Xếp hạng trong từng phân vùng

```rust
let rank = row_number().over(
    Window::new()
        .partition_by(Post::user_id)
        .order_by(Post::views.desc()),
);

Post::query().select((Post::title, rank.alias("position")));
```

```sql
SELECT "posts"."title",
       ROW_NUMBER() OVER (PARTITION BY "posts"."user_id" ORDER BY "posts"."views" DESC) AS "position"
FROM "posts"
```

Các hàm xếp hạng có sẵn: `row_number()`, `rank()`, `dense_rank()`, `percent_rank()`, `cume_dist()`, `ntile(buckets)`.

## 7.2. Hàm truy cập dòng khác

| Hàm | Ý nghĩa |
|---|---|
| `lag(expr, offset)` | giá trị của dòng lùi `offset` bước |
| `lead(expr, offset)` | giá trị của dòng tiến `offset` bước |
| `first_value(expr)` | giá trị đầu khung |
| `last_value(expr)` | giá trị cuối khung |
| `nth_value(expr, n)` | giá trị thứ `n` trong khung |

```rust
let previous = lag(Order::total, 1).over(
    Window::new()
        .partition_by(Order::user_id)
        .order_by(Order::placed_on.asc()),
);

Order::query().select((Order::id, previous.alias("previous_total")));
// SELECT "orders"."id",
//        LAG("orders"."total", $1) OVER (PARTITION BY "orders"."user_id"
//                                        ORDER BY "orders"."placed_on" ASC) AS "previous_total"
// FROM "orders"
```

## 7.3. Aggregate trên cửa sổ

Bất kỳ aggregate nào cũng dùng được với `.over(...)`, và nó trở thành một phép tính lũy tiến:

```rust
let running = Order::total.sum().over(
    Window::new()
        .partition_by(Order::user_id)
        .order_by(Order::placed_on.asc())
        .rows(FrameBound::UnboundedPreceding, Some(FrameBound::CurrentRow)),
);

Order::query().select((Order::id, running.alias("running_total")));
```

```sql
SELECT "orders"."id",
       SUM("orders"."total") OVER (
         PARTITION BY "orders"."user_id"
         ORDER BY "orders"."placed_on" ASC
         ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
       ) AS "running_total"
FROM "orders"
```

Cửa sổ rỗng phủ toàn bộ kết quả:

```rust
Order::total.sum().over(Window::new()).alias("grand_total");
// SUM("orders"."total") OVER () AS "grand_total"
```

## 7.4. Khung cửa sổ

Ba đơn vị khung, mỗi cái là một phương thức:

| Phương thức | Đơn vị |
|---|---|
| `.rows(start, end)` | đếm dòng vật lý |
| `.range(start, end)` | khoảng logic theo giá trị sắp xếp |
| `.groups(start, end)` | đếm nhóm ngang hàng |

`FrameBound` có `UnboundedPreceding`, `Preceding(n)`, `CurrentRow`, `Following(n)`, `UnboundedFollowing`.

Truyền `None` cho `end` sinh khung một phía, không có `BETWEEN`:

```rust
Post::views.sum().over(
    Window::new()
        .order_by(Post::published_at.asc())
        .rows(FrameBound::Preceding(2), None),
);
// SUM("posts"."views") OVER (ORDER BY "posts"."published_at" ASC ROWS 2 PRECEDING)
```

`.exclude(...)` thêm mệnh đề loại trừ, với `FrameExclusion` gồm `CurrentRow`, `Group`, `Ties`, `NoOthers`:

```rust
count_all().over(
    Window::new()
        .order_by(Post::views.asc())
        .range(FrameBound::UnboundedPreceding, Some(FrameBound::UnboundedFollowing))
        .exclude(FrameExclusion::CurrentRow),
);
// COUNT(*) OVER (ORDER BY "posts"."views" ASC
//                RANGE BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING
//                EXCLUDE CURRENT ROW)
```

## 7.5. Window đặt tên

Khai báo một lần, dùng lại nhiều chỗ:

```rust
Post::query()
    .window(
        "by_author",
        Window::new().partition_by(Post::user_id).order_by(Post::views.desc()),
    )
    .select((
        Post::title,
        row_number().over_named("by_author").alias("position"),
        Post::views.sum().over_named("by_author").alias("author_views"),
    ));
```

```sql
SELECT "posts"."title",
       ROW_NUMBER() OVER "by_author" AS "position",
       SUM("posts"."views") OVER "by_author" AS "author_views"
FROM "posts"
WINDOW "by_author" AS (PARTITION BY "posts"."user_id" ORDER BY "posts"."views" DESC)
```

Mệnh đề `WINDOW` cần `Caps::named_windows`; PostgreSQL, MySQL và SQLite đều có.

## 7.6. Thứ tự NULL bên trong cửa sổ

Cùng một cách giả lập như `ORDER BY` ở ngoài, áp dụng nhất quán:

```rust
Post::views.max().over(
    Window::new()
        .partition_by(Post::user_id)
        .order_by(Post::published_at.desc().nulls_last()),
);
```

```sql
-- PostgreSQL
MAX("posts"."views") OVER (PARTITION BY "posts"."user_id"
                           ORDER BY "posts"."published_at" DESC NULLS LAST)

-- SQLite
MAX("posts"."views") OVER (PARTITION BY "posts"."user_id"
                           ORDER BY CASE WHEN "posts"."published_at" IS NULL THEN 1 ELSE 0 END,
                                    "posts"."published_at" DESC)
```

## Bước tiếp theo

[Chương 8](./08-subqueries.md) lồng một query vào trong query khác.

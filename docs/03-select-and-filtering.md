---
id: select-and-filtering
title: 3. SELECT và lọc dữ liệu
sidebar_position: 3
---

# 3. SELECT và lọc dữ liệu

Chương này đi qua toàn bộ mệnh đề của một câu SELECT một bảng.
JOIN ở [chương 5](./05-joins.md), aggregate ở [chương 6](./06-aggregates-and-grouping.md).

Mọi ví dụ dùng entity sau:

```rust
#[derive(Entity)]
#[oxider(table = "users")]
struct User {
    id: i64,
    name: String,
    email: Option<String>,
    age: i32,
    active: bool,
    department_id: Option<i64>,
    created_at: NaiveDateTime,
}
```

## 3.1. Projection

Không gọi `select` thì projection là `SELECT *`:

```rust
User::query();
// SELECT * FROM "users"
```

`select` nhận một biểu thức, hoặc một tuple từ 2 tới 12 phần tử:

```rust
User::query().select((User::id, User::name, User::age));
// SELECT "users"."id", "users"."name", "users"."age" FROM "users"
```

Tuple một phần tử không tồn tại; viết thẳng biểu thức: `.select(User::id)`.

`add_select` nối thêm vào projection đã có, hữu ích khi dựng query theo nhánh:

```rust
let mut q = User::query().select(User::id);
if with_name {
    q = q.add_select(User::name);
}
```

Mọi biểu thức đều đặt được alias bằng `.alias(...)`:

```rust
User::query().select(User::name.upper().alias("shouted"));
// SELECT UPPER("users"."name") AS "shouted" FROM "users"
```

## 3.2. Lọc

`filter` nhận một `Predicate`.
Gọi nhiều lần thì các điều kiện được nối bằng `AND`:

```rust
User::query()
    .filter(User::age.ge(18))
    .filter(User::active.eq(true));
// SELECT * FROM "users" WHERE "users"."age" >= $1 AND "users"."active" = $2
```

Ngoặc do độ ưu tiên quyết định, không phải do template.
`OR` bên trong `AND` được bọc ngoặc, chiều ngược lại thì không:

```rust
User::query().filter(
    User::age.lt(18).or(User::age.gt(65)).and(User::active.eq(true))
);
// WHERE ("users"."age" < $1 OR "users"."age" > $2) AND "users"."active" = $3

User::query().filter(
    User::age.lt(18).and(User::active.eq(true)).or(User::age.gt(65))
);
// WHERE "users"."age" < $1 AND "users"."active" = $2 OR "users"."age" > $3
```

Bảng độ ưu tiên nằm ở một chỗ duy nhất trong tầng dialect, nên không có toán tử nào tự quyết định ngoặc của riêng nó.

Danh sách đầy đủ toán tử ở [chương 4](./04-operators.md).

## 3.3. Điều kiện tùy chọn

`filter_opt` bỏ qua điều kiện khi là `None`.
Đây là cách dựng bộ lọc động mà không phải nối chuỗi:

```rust
fn search(name: Option<&str>, min_age: Option<i32>) -> Select<Only<User>> {
    User::query()
        .filter_opt(name.map(|n| User::name.contains(n)))
        .filter_opt(min_age.map(|a| User::age.ge(a)))
}
```

Khi cả hai đều `None`, kết quả là `SELECT * FROM "users"`.

Nếu truyền thẳng `None` chứ không phải kết quả của `map`, trình biên dịch không suy được kiểu tập nguồn, nên phải chú thích:

```rust
let none: Option<Predicate<Only<User>>> = None;
User::query().filter_opt(none);
```

`Only<E>` là bí danh của tập chỉ chứa một entity.

Ở tầng biểu thức có `and_opt` và `or_opt` làm việc tương đương cho từng nhánh nhỏ.

## 3.4. Danh sách rỗng trong `IN`

`IN ()` không phải SQL hợp lệ.
Một danh sách rỗng trở thành hằng đúng nghĩa, thay vì im lặng khớp mọi dòng:

```rust
let ids: Vec<i64> = Vec::new();
User::query().filter(User::id.in_values(ids));
// SELECT * FROM "users" WHERE 1 = 0

User::query().filter(User::id.not_in_values(ids));
// SELECT * FROM "users" WHERE 1 = 1
```

Danh sách không rỗng bind một tham số cho mỗi giá trị:

```rust
User::query().filter(User::id.in_values([1i64, 2, 3]));
// SELECT * FROM "users" WHERE "users"."id" IN ($1, $2, $3)
```

## 3.5. Sắp xếp

`order_by` nối thêm một tiêu chí, theo thứ tự gọi:

```rust
User::query().order_by(User::age.desc()).order_by(User::name.asc());
// ORDER BY "users"."age" DESC, "users"."name" ASC
```

`order_by_all` nhận một iterator, `order_by_opt` bỏ qua khi `None`.

### Vị trí của NULL

`nulls_first()` và `nulls_last()` là native trên PostgreSQL, và được giả lập trên MySQL và SQLite bằng một khóa sắp xếp phụ:

```rust
User::query().order_by(User::email.desc().nulls_last());
```

```sql
-- PostgreSQL
ORDER BY "users"."email" DESC NULLS LAST

-- MySQL và SQLite
ORDER BY CASE WHEN "users"."email" IS NULL THEN 1 ELSE 0 END, "users"."email" DESC
```

Giả lập cho kết quả sắp xếp giống hệt bản native, và điều đó được kiểm chứng bằng test chạy trên database thật chứ không chỉ so chuỗi SQL.

## 3.6. Phân trang

```rust
User::query().limit(25);           // LIMIT 25
User::query().offset(10);          // OFFSET 10
User::query().page(2, 25);         // LIMIT 25 OFFSET 50
```

`page(page, size)` đánh số trang từ 1.

`OFFSET` không kèm `LIMIT` là hợp lệ trên PostgreSQL nhưng không trên MySQL và SQLite, nên hai engine đó nhận một `LIMIT` giữ chỗ:

```sql
-- PostgreSQL
SELECT * FROM "users" OFFSET 10
-- MySQL
SELECT * FROM `users` LIMIT 18446744073709551615 OFFSET 10
-- SQLite
SELECT * FROM "users" LIMIT -1 OFFSET 10
```

## 3.7. DISTINCT

```rust
User::query().distinct().select(User::department_id);
// SELECT DISTINCT "users"."department_id" FROM "users"
```

`distinct_on` chỉ có trên PostgreSQL và bị từ chối ở nơi khác:

```rust
let query = User::query()
    .distinct_on(User::department_id)
    .select((User::department_id, User::name))
    .order_by(User::department_id.asc())
    .order_by(User::name.asc());

query.to_sql(&Postgres).unwrap();
// SELECT DISTINCT ON ("users"."department_id") ... 

query.to_sql(&MySql).unwrap_err();
// UnsupportedFeature { dialect: "mysql", feature: "DISTINCT ON" }
```

## 3.8. Khóa dòng

```rust
User::query().filter(User::id.eq(1)).for_update().skip_locked();
// PostgreSQL và MySQL:
//   SELECT * FROM "users" WHERE "users"."id" = $1 FOR UPDATE SKIP LOCKED
// SQLite: bị từ chối, "row locking"
```

Có `for_update`, `for_share`, `for_no_key_update`, `for_key_share`, kèm hai modifier `no_wait()` và `skip_locked()`.
Bốn dạng khóa không có ở mọi engine, và `no_wait`/`skip_locked` cần `Caps::lock_wait_policy`; xem [chương 11](./11-dialects.md).

## 3.9. SELECT không có FROM

```rust
oxider_query::select_only(val(1i64).alias("one"));
// SELECT $1 AS "one"
```

Trên các engine đòi hỏi một mệnh đề `FROM`, dialect tự thêm bảng giả của nó.

`select_from_name("tên")` bắt đầu một SELECT từ một nguồn chỉ có tên, dùng để đọc CTE; xem [chương 9](./09-set-operations-and-ctes.md).

## 3.10. Cột thời gian

Với feature `chrono` (mặc định bật), so sánh trực tiếp với giá trị `chrono`:

```rust
let t = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap().and_hms_opt(9, 30, 0).unwrap();
User::query().filter(User::created_at.ge(t));
// SELECT * FROM "users" WHERE "users"."created_at" >= $1
// params: [DateTime("2024-03-15 09:30:00")]
```

Giá trị thời gian bind dưới dạng text ISO-8601 chứ không phải kiểu native của driver.
Mọi engine đọc được dạng này trong ngữ cảnh ngày giờ, chuỗi giống nhau trên cả ba dialect, và core không phải phụ thuộc vào driver nào.
Lớp thực thi mới là nơi quyết định bind `Value::DateTime` thành kiểu gì.

Toán tử ngày giờ ở [mục 4.5](./04-operators.md).

## Bước tiếp theo

[Chương 4](./04-operators.md) liệt kê toàn bộ toán tử dùng được bên trong `filter` và `select`.

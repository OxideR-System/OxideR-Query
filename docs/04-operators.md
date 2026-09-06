---
id: operators
title: 4. Bộ toán tử
sidebar_position: 4
---

# 4. Bộ toán tử

QueryDSL phân tầng toán tử bằng cây kế thừa lớp: `SimpleExpression`, `ComparableExpression`, `NumberExpression`, `StringExpression`.
OxideR-Query đạt cùng hiệu quả bằng trait bound trên kiểu `T` của biểu thức.

| Lớp QueryDSL | Trait ở đây | Bound |
|---|---|---|
| `SimpleExpression` | `CompareOps<T>` | `T: SqlType` |
| `ComparableExpression` | `OrderOps<T>` | `T: Orderable` |
| `NumberExpression` | `MathOps<T>` | `T: Numeric` |
| `StringExpression` | `TextOps` | `T = String` |
| `BooleanExpression` | `BoolOps` | `T = bool` |
| `DateTimeExpression` | `TemporalOps<T>` | `T: Temporal` |

Mọi trait này đều nằm trong `prelude`.
Chúng áp dụng cho cả `Column` lẫn `Expr`, nên toán tử ghép chuỗi được không giới hạn.

Mọi toán hạng nhận `impl IntoExpr<T>`, nghĩa là chỗ nào nhận giá trị thì cũng nhận cột hoặc biểu thức khác.

## 4.1. So sánh và null

| Phương thức | SQL |
|---|---|
| `eq(x)` | `self = x` |
| `ne(x)` | `self <> x` |
| `eq_column(x)` | `self = x`, tên gọi rõ nghĩa cho khóa join |
| `lt(x)`, `le(x)`, `gt(x)`, `ge(x)` | `<`, `<=`, `>`, `>=` |
| `between(a, b)` | `self BETWEEN a AND b` |
| `not_between(a, b)` | `self NOT BETWEEN a AND b` |
| `is_null()` | `self IS NULL` |
| `is_not_null()` | `self IS NOT NULL` |
| `is_not_distinct_from(x)` | so sánh an toàn với NULL |
| `is_distinct_from(x)` | phủ định của trên |
| `in_values(iter)` | `self IN (...)` |
| `not_in_values(iter)` | `self NOT IN (...)` |
| `asc()`, `desc()` | tiêu chí `ORDER BY` |

`lt`/`le`/`gt`/`ge`/`between`/`asc`/`desc` cần `T: Orderable`, phần còn lại chỉ cần `T: SqlType`.

So sánh an toàn với NULL viết khác nhau ở mỗi engine, và đó là điều thư viện lo giúp:

```rust
User::query().filter(User::manager_id.is_not_distinct_from(User::department_id));
```

```sql
-- PostgreSQL
WHERE "users"."manager_id" IS NOT DISTINCT FROM "users"."department_id"
-- MySQL
WHERE `users`.`manager_id` <=> `users`.`department_id`
-- SQLite
WHERE "users"."manager_id" IS "users"."department_id"
```

## 4.2. Boolean

| Phương thức | SQL |
|---|---|
| `and(p)` | `self AND p` |
| `or(p)` | `self OR p` |
| `xor(p)` | `self XOR p` |
| `not()` | `NOT self` |
| `and_opt(Some/None)` | nối khi có, giữ nguyên khi không |
| `or_opt(Some/None)` | như trên |

Cột `bool` dùng trực tiếp làm điều kiện, không cần so với `true`:

```rust
User::query().filter(User::active.not().and(User::age.gt(65)));
// WHERE NOT "users"."active" AND "users"."age" > $1
```

`XOR` là từ khóa native của MySQL; các engine khác diễn đạt bằng bất đẳng thức.
Vì `<>` có độ ưu tiên khác từ khóa `XOR`, ngoặc được thêm đúng chỗ ở mỗi bên:

```rust
User::query().filter(User::active.xor(User::email.is_null()));
```

```sql
-- PostgreSQL
WHERE "users"."active" <> ("users"."email" IS NULL)
-- MySQL
WHERE `users`.`active` XOR `users`.`email` IS NULL
```

## 4.3. Chuỗi

### So khớp mẫu

| Phương thức | Ý nghĩa |
|---|---|
| `contains(v)` | chứa `v`, `v` được escape |
| `starts_with(v)` | bắt đầu bằng `v`, escape |
| `ends_with(v)` | kết thúc bằng `v`, escape |
| `contains_ignore_case(v)` và hai biến thể `_ignore_case` còn lại | như trên, không phân biệt hoa thường |
| `like(pattern)` | mẫu thô, không escape |
| `like_ignore_case(pattern)` | mẫu thô, không phân biệt hoa thường |
| `matches(pattern)` | so khớp biểu thức chính quy |
| `matches_ignore_case(pattern)` | như trên, không phân biệt hoa thường |
| `eq_ignore_case(x)` | bằng nhau không phân biệt hoa thường |
| `is_empty()` | `self = ''` |

Đây là chỗ OxideR-Query sửa một lỗi của QueryDSL.
QueryDSL chỉ escape toán hạng hằng và bỏ qua toán hạng động, nên tìm `50%` ở đó khớp cả `500 units`.
Ở đây việc escape là vô điều kiện, và mệnh đề `ESCAPE` luôn được phát ra để ghim ký tự escape:

```rust
Post::query().filter(Post::title.contains("50%"));
// SELECT * FROM "posts" WHERE "posts"."title" LIKE $1 ESCAPE '!'
// params: ["%50!%%"]

Post::query().filter(Post::title.starts_with("a_b!c"));
// params: ["a!_b!!c%"]
```

Ký tự escape là `!` chứ không phải dấu chéo ngược.
Dấu chéo ngược mang nghĩa khác nhau bên trong chuỗi ký tự tùy engine, trong khi `!` thì không.

Cần mẫu thô, ví dụ khi mẫu do người dùng nhập có chủ ý wildcard, thì dùng `like`:

```rust
Post::query().filter(Post::title.like("draft%"));
// WHERE "posts"."title" LIKE $1        (không có ESCAPE)
```

Không phân biệt hoa thường dùng `ILIKE` của PostgreSQL, và `LOWER` ở nơi khác:

```sql
-- PostgreSQL
WHERE "posts"."title" ILIKE $1 ESCAPE '!'
-- MySQL
WHERE LOWER(`posts`.`title`) LIKE LOWER(?) ESCAPE '!'
```

### Hàm chuỗi

| Phương thức | SQL |
|---|---|
| `upper()`, `lower()` | `UPPER`, `LOWER` |
| `trim()`, `trim_start()`, `trim_end()` | cắt khoảng trắng |
| `length()` | độ dài, trả `i64` |
| `substr(start)` | cắt từ vị trí |
| `substr_len(start, len)` | cắt theo độ dài |
| `left(n)`, `right(n)` | n ký tự đầu hoặc cuối |
| `index_of(needle)` | vị trí xuất hiện, trả `i64` |
| `pad_start(n, fill)`, `pad_end(n, fill)` | đệm tới độ dài `n` |
| `replace(from, to)` | thay thế |
| `concat(x)` | nối chuỗi |

Cùng một ý nghĩa, cú pháp khác nhau ở mỗi engine:

```rust
User::query().select(User::name.substr_len(2, 5));
```

```sql
-- PostgreSQL
SELECT SUBSTRING("users"."name" FROM $1 FOR $2) FROM "users"
-- MySQL
SELECT SUBSTRING(`users`.`name`, ?, ?) FROM `users`
-- SQLite
SELECT SUBSTR("users"."name", ?, ?) FROM "users"
```

`index_of` còn phải đảo thứ tự tham số:

```sql
-- PostgreSQL
POSITION($1 IN "users"."name")
-- MySQL
LOCATE(?, `users`.`name`)
-- SQLite
INSTR("users"."name", ?)
```

Nối chuỗi là toán tử ở mọi nơi trừ MySQL, nơi nó là hàm:

```sql
-- PostgreSQL, SQLite
"users"."name" || "users"."email"
-- MySQL
CONCAT(`users`.`name`, `users`.`email`)
```

`pad_start`/`pad_end` không có trên SQLite và bị từ chối ở đó.

## 4.4. Số học

| Nhóm | Phương thức |
|---|---|
| Bốn phép tính | `add`, `sub`, `mul`, `div`, `rem` |
| Cùng kiểu | `abs`, `ceil`, `floor`, `round`, `negate`, `sign` |
| Trả `f64` | `sqrt`, `exp`, `ln`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `cot`, `coth`, `degrees`, `radians` |
| Hai toán hạng | `power(exponent)`, `log(base)` |
| Làm tròn theo chữ số | `round_to(digits)` |

Ngoặc do độ ưu tiên quyết định, nên biểu thức chỉ có ngoặc khi thực sự cần:

```rust
OrderItem::price.mul(2.0).add(1.0);
// "order_items"."price" * $1 + $2

OrderItem::price.add(1.0).mul(2.0);
// ("order_items"."price" + $1) * $2
```

Phép chia lấy dư là toán tử trên PostgreSQL và SQLite, là hàm trong ANSI và MySQL:

```sql
-- PostgreSQL, SQLite
"posts"."views" % $1 = $2
-- MySQL
MOD(`posts`.`views`, ?) = ?
```

## 4.5. Toán tử ngày giờ

Cần `T: Temporal`, tức là các kiểu `chrono` khi feature `chrono` bật.

| Nhóm | Phương thức |
|---|---|
| Trích thành phần | `year`, `month`, `day`, `hour`, `minute`, `second`, `millisecond`, `week`, `day_of_week`, `day_of_year`, `year_month`, `year_week` |
| Cộng khoảng | `add_years`, `add_months`, `add_weeks`, `add_days`, `add_hours`, `add_minutes`, `add_seconds` |
| Cắt về đầu kỳ | `truncate_to_year`, `truncate_to_month`, `truncate_to_week`, `truncate_to_day`, `truncate_to_hour`, `truncate_to_minute`, `truncate_to_second` |
| Hiệu | `diff_years`, `diff_months`, `diff_days`, `diff_hours`, `diff_minutes`, `diff_seconds` |
| Khác | `date()` lấy phần ngày |

Trích thành phần đọc giống nhau ở tầng Rust, khác hẳn ở tầng SQL:

```rust
User::query().select(User::created_at.year());
```

```sql
-- PostgreSQL
EXTRACT(YEAR FROM "users"."created_at")
-- MySQL
YEAR(`users`.`created_at`)
-- SQLite
CAST(STRFTIME('%Y', "users"."created_at") AS INTEGER)
```

`day_of_week` được chuẩn hóa để Chủ nhật luôn là 1 trên cả ba engine, bằng cách cộng bù ở nơi cần:

```sql
-- PostgreSQL
(EXTRACT(DOW FROM "users"."created_at") + 1)
-- MySQL
DAYOFWEEK(`users`.`created_at`)
-- SQLite
(CAST(STRFTIME('%w', "users"."created_at") AS INTEGER) + 1)
```

Cộng khoảng dùng cú pháp interval riêng của từng engine:

```sql
-- PostgreSQL
("users"."created_at" + ($1 * INTERVAL '1 day'))
-- MySQL
DATE_ADD(`users`.`created_at`, INTERVAL ? DAY)
-- SQLite
DATETIME("users"."created_at", ? || ' days')
```

Cắt về đầu tháng được giả lập ở nơi không có `DATE_TRUNC`:

```sql
-- PostgreSQL
DATE_TRUNC('month', "users"."created_at")
-- MySQL
DATE_SUB(DATE(`users`.`created_at`), INTERVAL DAYOFMONTH(`users`.`created_at`) - 1 DAY)
-- SQLite
DATE("users"."created_at", 'start of month')
```

Hằng thời gian hiện tại không bind tham số nào:

```rust
User::query().filter(User::created_at.lt(now()));
// WHERE "users"."created_at" < CURRENT_TIMESTAMP
```

Có `now()`, `today()` (cần feature `chrono`), cùng `current_date()`, `current_time()`, `current_timestamp()` dạng tổng quát theo kiểu.

## 4.6. Biểu thức điều kiện

### CASE

```rust
let bucket = case_when(User::age.lt(18), "minor")
    .when(User::age.lt(65), "adult")
    .otherwise("senior");

User::query().select((User::name, bucket.alias("bracket")));
// SELECT "users"."name",
//        CASE WHEN "users"."age" < $1 THEN $2
//             WHEN "users"."age" < $3 THEN $4
//             ELSE $5 END AS "bracket"
// FROM "users"
```

Không có `otherwise` thì kết thúc bằng `end()`, và dòng không khớp nhánh nào cho `NULL`:

```rust
let flagged = case_when(User::active.eq(true), 1i64).end();
// CASE WHEN "users"."active" = $1 THEN $2 END
```

### COALESCE, NULLIF, LEAST, GREATEST

`coalesce`, `least`, `greatest` nhận toán hạng đầu rồi nối tiếp bằng `.or(...)`, kết thúc bằng `.end()`:

```rust
User::query().select(coalesce(User::email).or("none@example.com").end());
// SELECT COALESCE("users"."email", $1) FROM "users"

User::query().select(nullif(User::name, ""));
// SELECT NULLIF("users"."name", $1) FROM "users"
```

`LEAST`/`GREATEST` mang tên khác trên SQLite:

```sql
-- PostgreSQL, MySQL
LEAST("order_items"."price", $1)
-- SQLite
MIN("order_items"."price", ?)
```

## 4.7. CAST và ép kiểu

`cast::<U>(kind)` phát ra một `CAST` thật, với tên kiểu đích do dialect quyết định:

```rust
User::query().select(User::age.cast::<String>(CastKind::Text));
```

```sql
-- PostgreSQL, SQLite
CAST("users"."age" AS TEXT)
-- MySQL
CAST(`users`.`age` AS CHAR)
```

`CastKind` có `Integer`, `Float`, `Decimal`, `Text`, `Bool`, `Date`, `Time`, `DateTime`.

`coerce::<U>()` chỉ đổi kiểu ở tầng Rust và không sinh SQL nào.
Dùng khi bạn biết chắc kiểu thật của một biểu thức mà hệ thống kiểu không suy ra được, ví dụ sau một `raw`.

Cả `cast` và `coerce` đến từ trait `ExprExt`, áp dụng được cho mọi biểu thức kể cả `Column`.

## 4.8. Hàm và hằng dựng sẵn

| Hàm | Ý nghĩa |
|---|---|
| `val(x)` | một giá trị làm biểu thức |
| `null::<T>()` | hằng `NULL` có kiểu |
| `param::<T>("tên")` | tham số đặt tên, cho dialect hỗ trợ |
| `col::<T>(qualifier, name)` | cột không thuộc entity nào |
| `star()`, `star_of(qualifier)` | `*` và `t.*` |
| `all_of(column)` | `t.*` suy từ bảng của cột |
| `random()` | số ngẫu nhiên |
| `next_val(seq)`, `curr_val(seq)` | sequence, PostgreSQL |
| `round_to(x, digits)` | làm tròn theo số chữ số |

### 4.8.1. Cột không thuộc entity

`col::<T>(qualifier, name)` tạo tham chiếu cột thô.
Dùng cho CTE, derived table, và mọi nguồn không có entity mô tả:

```rust
col::<i64>("totals", "user_id")
// "totals"."user_id"
```

Biểu thức này có tập nguồn rỗng, nên nó không được kiểm tra phạm vi.
Đó là cái giá phải trả để nói về một nguồn mà hệ thống kiểu không biết.

## 4.9. Raw SQL có tham số

Khi cần thứ thư viện chưa mô hình hóa, ví dụ toán tử JSON của PostgreSQL:

```rust
use oxider_query::typed::Raw;

let json_path = Raw::new()
    .expr(Post::body)
    .sql(" ->> ")
    .bind("author")
    .build::<String>();

Post::query().select(json_path.alias("author"));
// SELECT "posts"."body" ->> $1 AS "author" FROM "posts"
```

`Raw` ghép ba loại mảnh: `sql(text)` viết SQL nguyên văn, `expr(e)` nhúng một biểu thức đã dựng, `bind(v)` thêm một tham số.

Điểm quan trọng: `bind` vẫn đi qua đường tham số hóa bình thường, nên đánh số placeholder vẫn đúng và không có nguy cơ injection.
Tập nguồn của các `expr` được cộng dồn, nên `Raw` vẫn chịu kiểm tra phạm vi bảng.

Cần đúng một chuỗi SQL không tham số thì `raw::<T>("...")` là dạng ngắn.

## Bước tiếp theo

[Chương 5](./05-joins.md) mở rộng query ra nhiều bảng.

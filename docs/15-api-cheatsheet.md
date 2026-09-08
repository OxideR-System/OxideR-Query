---
id: api-cheatsheet
title: 15. Tra cứu nhanh API
sidebar_position: 15
---

# 15. Tra cứu nhanh API

Bảng tra cứu một trang.
Giải thích chi tiết ở các chương tương ứng.

## Entity

```rust
#[derive(Entity)]
#[oxider(table = "users", schema = "app")]
struct User {
    id: i64,
    #[oxider(column = "full_name")]
    name: String,
    email: Option<String>,
    #[oxider(skip)]
    computed: String,
}
```

| Mục | Ý nghĩa |
|---|---|
| `#[oxider(table = "...")]` | tên bảng, mặc định là tên struct viết thường |
| `#[oxider(schema = "...")]` | qualifier schema |
| `#[oxider(column = "...")]` | tên cột, mặc định là tên field |
| `#[oxider(skip)]` | bỏ field khỏi metamodel |

| Sinh ra | Ý nghĩa |
|---|---|
| `User::TABLE`, `User::SCHEMA` | hằng |
| `User::query()` | bắt đầu SELECT |
| `User::query_as("u")` | SELECT từ bảng đã đặt alias |
| `User::table()` | bảng làm đích join |
| `User::insert()`, `User::update()`, `User::delete()` | bắt đầu DML |
| `User::id`, `User::name`, ... | `Column<User, T>` |

## Bắt đầu một query

| Hàm | Ý nghĩa |
|---|---|
| `E::query()` | `SELECT ... FROM bảng của E` |
| `E::query_as(alias)` | như trên, có alias |
| `select_from(table)` | như `E::query()`, dạng hàm tự do |
| `select_from_name(name)` | FROM một nguồn chỉ có tên (CTE) |
| `select_only(sel)` | SELECT không có FROM |

## Mệnh đề của `Select`

| Nhóm | Phương thức |
|---|---|
| Projection | `select`, `add_select`, `distinct`, `distinct_on` |
| Nguồn | `and_from`, `and_from_query`, `and_from_name` |
| Join | `inner_join`, `left_join`, `right_join`, `full_join`, `cross_join`, `join_query`, `join_name`, `join_name_as` |
| Lọc | `filter`, `filter_opt` |
| Nhóm | `group_by`, `having` |
| Cửa sổ | `window(name, w)` |
| Sắp xếp | `order_by`, `order_by_all`, `order_by_opt` |
| Phân trang | `limit`, `offset`, `page` |
| Khóa | `for_update`, `for_share`, `for_no_key_update`, `for_key_share`, `no_wait`, `skip_locked` |
| CTE | `with`, `with_columns`, `recursive` |
| Set op | `union`, `union_all`, `intersect`, `intersect_all`, `except`, `except_all` |
| Tương quan | `correlate::<E>()` |
| Thành biểu thức | `scalar(expr)`, `as_subquery()` |
| Render | `to_sql(&dialect)` |

## Toán tử so sánh

`eq` `ne` `eq_column` `lt` `le` `gt` `ge` `between` `not_between` `is_null` `is_not_null` `is_distinct_from` `is_not_distinct_from` `in_values` `not_in_values` `asc` `desc`

`asc`/`desc` có thêm `.nulls_first()` và `.nulls_last()`.

## Toán tử boolean

`and` `or` `xor` `not` `and_opt` `or_opt`

## Toán tử chuỗi

So khớp: `like` `like_ignore_case` `contains` `contains_ignore_case` `starts_with` `starts_with_ignore_case` `ends_with` `ends_with_ignore_case` `matches` `matches_ignore_case` `eq_ignore_case` `is_empty`

Hàm: `upper` `lower` `trim` `trim_start` `trim_end` `length` `substr` `substr_len` `left` `right` `index_of` `pad_start` `pad_end` `replace` `concat`

## Toán tử số học

Phép tính: `add` `sub` `mul` `div` `rem` `power` `log`

Cùng kiểu: `abs` `ceil` `floor` `round` `negate` `sign` `round_to`

Trả `f64`: `sqrt` `exp` `ln` `sin` `cos` `tan` `asin` `acos` `atan` `sinh` `cosh` `tanh` `cot` `coth` `degrees` `radians`

## Toán tử ngày giờ

Trích: `year` `month` `day` `hour` `minute` `second` `millisecond` `week` `day_of_week` `day_of_year` `year_month` `year_week`

Cộng: `add_years` `add_months` `add_weeks` `add_days` `add_hours` `add_minutes` `add_seconds`

Cắt: `truncate_to_year` `truncate_to_month` `truncate_to_week` `truncate_to_day` `truncate_to_hour` `truncate_to_minute` `truncate_to_second`

Hiệu: `diff_years` `diff_months` `diff_days` `diff_hours` `diff_minutes` `diff_seconds`

Khác: `date`

## Aggregate

| Hàm | Kiểu trả về |
|---|---|
| `count_all()` | `i64` |
| `x.count()`, `x.count_distinct()` | `i64` |
| `x.min()`, `x.max()` | `T` |
| `x.sum()` | `T` |
| `x.avg()` | `f64` |
| `x.std_dev()`, `x.std_dev_pop()` | `f64` |
| `x.variance()`, `x.var_pop()` | `f64` |
| `bool_and(x)`, `bool_or(x)` | `bool` |
| `group_concat(x, sep)` | `String` |

Bổ nghĩa cho aggregate: `.distinct()`, `.filter_where(pred)`, `.order_by(term)`, `.alias(name)`, `.over(window)`, `.over_named(name)`.

## Window function

Hàm: `row_number()` `rank()` `dense_rank()` `percent_rank()` `cume_dist()` `ntile(n)` `lag(x, n)` `lead(x, n)` `first_value(x)` `last_value(x)` `nth_value(x, n)`

Dựng cửa sổ:

```rust
Window::new()
    .partition_by(expr)
    .order_by(term)
    .rows(FrameBound::UnboundedPreceding, Some(FrameBound::CurrentRow))
    .exclude(FrameExclusion::CurrentRow)
```

`.rows` / `.range` / `.groups`.
`FrameBound`: `UnboundedPreceding`, `Preceding(n)`, `CurrentRow`, `Following(n)`, `UnboundedFollowing`.
`FrameExclusion`: `CurrentRow`, `Group`, `Ties`, `NoOthers`.

## Subquery

| Cách viết | Ý nghĩa |
|---|---|
| `q.scalar(expr)` | thành `Subquery<F, T>` |
| `exists(q)`, `not_exists(q)` | thành `Predicate<F>` |
| `x.in_subquery(sq)`, `x.not_in_subquery(sq)` | membership |
| `sq.any()`, `sq.all()` | lượng từ |
| `q.correlate::<E>()` | đưa entity ngoài vào phạm vi |

## Hàm và biểu thức dựng sẵn

`val(x)` `null::<T>()` `param::<T>(name)` `col::<T>(qual, name)` `star()` `star_of(qual)` `all_of(column)` `random()` `next_val(seq)` `curr_val(seq)` `current_date()` `current_time()` `current_timestamp()` `now()` `today()` `nullif(a, b)` `round_to(x, digits)` `raw::<T>(sql)`

Dạng nối tiếp, kết thúc bằng `.end()`:
`coalesce(x).or(y).end()` `least(x).or(y).end()` `greatest(x).or(y).end()`

CASE:
`case_when(cond, val).when(cond, val).otherwise(val)` hoặc `.end()`

Raw có tham số:
`Raw::new().expr(e).sql(" ->> ").bind(v).build::<T>()`

Trên mọi biểu thức (trait `ExprExt`):
`.alias(name)` `.cast::<U>(kind)` `.coerce::<U>()` `.into_expr()`

## DML

| Builder | Phương thức |
|---|---|
| `Insert` | `set`, `columns`, `values`, `from_query`, `on_conflict`, `returning` |
| sau `on_conflict` | `do_nothing()`, `do_update().set(...).end()` |
| trong `do_update` | `excluded(column)` |
| `Update` | `set`, `set_null`, `from`, `filter`, `filter_opt`, `returning` |
| `Delete` | `using`, `filter`, `filter_opt`, `returning` |

## Render và thực thi

```rust
let rendered = query.to_sql(&Postgres)?;   // Rendered { sql, params }
```

Mọi builder đều có `bind(name, value)`, điền giá trị cho một `param::<T>(name)` ở bất kỳ đâu trong câu lệnh.

```rust
let mau = User::query().filter(User::age.ge(param::<i32>("tuoi")));
mau.clone().bind("tuoi", 18).to_sql(&Postgres)?;
mau.bind("tuoi", 21).to_sql(&Postgres)?;
```

| Kiểu | Ý nghĩa |
|---|---|
| `Rendered` | `{ sql: String, params: Vec<Value> }` |
| `RenderError` | `UnsupportedOperator`, `UnsupportedFeature`, `MissingArgument`, `Invalid`, `TooDeep`, `UnboundParameter` |
| `Dialect` | `Postgres`, `MySql`, `Sqlite` |

```rust
let db = SqliteDb::connect(url).await?;
db.execute(stmt).await?;                    // u64
db.fetch_all::<Row, _>(query).await?;       // Vec<Row>
db.fetch_one::<Row, _>(query).await?;       // Row
db.fetch_optional::<Row, _>(query).await?;  // Option<Row>
db.transaction(async |tx| { ... }).await?;
```

## Kiểu ở tầng type

| Kiểu | Ý nghĩa |
|---|---|
| `Column<E, T>` | cột của entity `E`, kiểu `T` |
| `Aliased<E>` | bản sao có alias của `E`, một entity riêng |
| `Expr<S, T>` | biểu thức tham chiếu tập `S`, kiểu `T` |
| `Predicate<S>` | bí danh của `Expr<S, bool>` |
| `Order<S>` | tiêu chí `ORDER BY` |
| `Aggregate<S, T>` | aggregate chưa gắn vào query |
| `Window<S>` | định nghĩa cửa sổ |
| `Select<S, F>` | `S` là phạm vi, `F` là entity tự do |
| `Subquery<F, T>` | subquery trả `T`, tự do trong `F` |
| `Only<E>` | tập chỉ chứa một entity |
| `Merge<A, B>` | tập ghép của hai tập |

Các marker type-state, cần khi viết chữ ký hàm trả về một statement. Tất cả đều có trong prelude.

| Kiểu | Nghĩa |
|---|---|
| `Unlocked` / `Locked` | `Select` đã có mệnh đề khóa dòng hay chưa |
| `NoRows` / `OneRow` / `FromQuery` | `Insert` lấy dữ liệu từ đâu |
| `NoFrame` / `Framed` | `Window` đã đặt frame hay chưa |

```rust
fn new_user(name: &str) -> Insert<User, OneRow> { ... }
fn next_job() -> Select<Only<Job>, Nil, Locked> { ... }
fn trailing_total() -> Window<Nil, Framed> { ... }
```

## Bước tiếp theo

[Chương 16](./16-security-model.md) mô tả ranh giới tin cậy: cái gì được bind, cái gì được nội suy, và bạn chịu trách nhiệm ở đâu.

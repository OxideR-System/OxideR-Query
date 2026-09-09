---
id: dialects
title: 11. Dialect
sidebar_position: 11
---

# 11. Dialect

Một query dựng ra là một AST độc lập engine.
Dialect là thứ biến AST đó thành SQL cụ thể, và là thứ quyết định query nào không diễn đạt được.

## 11.1. Ba dialect có sẵn

```rust
use oxider_query::{Postgres, MySql, Sqlite};

let query = User::query().filter(User::age.ge(18));
query.to_sql(&Postgres)?;
query.to_sql(&MySql)?;
query.to_sql(&Sqlite)?;
```

Khác biệt cơ bản nhất:

| | Trích dẫn định danh | Placeholder |
|---|---|---|
| PostgreSQL | `"users"."id"` | `$1`, `$2`, ... |
| MySQL | `` `users`.`id` `` | `?` |
| SQLite | `"users"."id"` | `?` |

Ký tự trích dẫn bên trong định danh được nhân đôi để escape, nên tên bảng có dấu nháy vẫn an toàn.

## 11.2. Template: bảng tra thay vì match cứng

Đây là điểm mấu chốt của kiến trúc, và là thứ được học thẳng từ QueryDSL.

Renderer không biết gì về từng toán tử.
Nó tra bảng `Operator -> Template` rồi điền tham số vào chỗ trống.

```rust
pub enum Elem {
    Lit(&'static str),   // văn bản SQL nguyên văn
    Arg(u8),             // toán hạng thứ n, render tại chỗ
    Ident(u8),           // toán hạng thứ n như một định danh trần
    Rest(u8),            // mọi toán hạng còn lại, ngăn bằng dấu phẩy
}

pub struct Template(pub &'static [Elem]);
```

Bảng ANSI phục vụ mặc định, mỗi dialect chỉ override những dòng khác biệt:

```rust
// ANSI
Least => t![L("LEAST("), R(0), L(")")]
// SQLite override
Least => t![L("MIN("), R(0), L(")")]
```

QueryDSL làm cùng việc này bằng chuỗi template `"{0} = {1}"` phân tích bằng regex lúc chạy.
Ở đây chỉ số toán hạng được kiểm tra ngay khi viết bảng, không có bước parse, và bảng vẫn là một danh sách phẳng dễ diff.

Hệ quả thực tế: thêm một toán tử là thêm một dòng, không sửa renderer; thêm một dialect là override vài dòng, không nhân bản logic.

## 11.3. Độ ưu tiên quyết định ngoặc

Ngoặc không nằm trong template.
Nó được quyết định một lần, tập trung, từ một thang độ ưu tiên, đúng như QueryDSL quyết định trong `SerializerBase.visitOperation`.

Thang từ chặt tới lỏng:

| Mức | Nội dung |
|---|---|
| `HIGHEST` | dạng lời gọi hàm, không bao giờ cần ngoặc |
| `DOT` | truy cập thành phần |
| `NOT_HIGH` | phủ định boolean dạng chặt |
| `NEGATE` | đổi dấu số học |
| `ARITH_HIGH` | nhân, chia, lấy dư |
| `ARITH_LOW` | cộng, trừ |
| `COMPARISON` | `<`, `<=`, `>`, `>=`, `BETWEEN`, `IN`, `LIKE` |
| `EQUALITY` | `=`, `<>` |
| `IS` | `IS NULL` và họ hàng |
| `CASE` | nhánh `CASE`, danh sách ngăn bằng phẩy |
| `NOT` | `NOT` |
| `AND` | `AND` |
| `XOR` | `XOR` |
| `OR` | `OR` |

Một toán hạng được bọc ngoặc khi và chỉ khi nó lỏng hơn toán tử bao ngoài.

Vì sao điều này quan trọng: mức ưu tiên phải khớp với văn bản **thực sự được phát ra**, không phải với ý niệm trừu tượng.
Template ANSI của `XOR` là `<>`, nên mức của nó phải là `EQUALITY`; nhưng MySQL phát ra từ khóa `XOR` nên nó override thành mức `XOR`.
Không làm vậy thì `active XOR (email IS NULL)` sẽ mất ngoặc trên PostgreSQL và được database phân tích thành một câu khác hẳn.

## 11.4. Caps: engine này làm được gì

```rust
pub struct Caps {
    pub nulls_ordering: bool,
    pub distinct_on: bool,
    pub aggregate_filter: bool,
    pub ordered_set_aggregates: bool,
    pub window_functions: bool,
    pub named_windows: bool,
    pub cte: bool,
    pub recursive_cte: bool,
    pub returning: bool,
    pub right_join: bool,
    pub full_join: bool,
    pub intersect: bool,
    pub except: bool,
    pub set_op_all: bool,
    pub row_locking: bool,
    pub lock_wait_policy: bool,
    pub on_conflict: bool,
    pub on_duplicate_key: bool,
    pub multi_row_insert: bool,
    pub wrap_set_op_branches: bool,
}
```

Giá trị thực tế:

| Khả năng | PostgreSQL | MySQL | SQLite |
|---|:-:|:-:|:-:|
| `NULLS FIRST/LAST` native | có | không | không |
| `DISTINCT ON` | có | không | không |
| `FILTER` trên aggregate | có | không | có |
| `WITHIN GROUP` (percentile) | có | không | không |
| Window function | có | có | có |
| Mệnh đề `WINDOW` | có | có | có |
| CTE, CTE đệ quy | có | có | có |
| `RETURNING` | có | không | có |
| `RIGHT JOIN` | có | có | không |
| `FULL JOIN` | có | không | không |
| `INTERSECT`, `EXCEPT` | có | không | có |
| Dạng `ALL` của set op | có | không | không |
| Khóa dòng | có | có | không |
| `SKIP LOCKED` / `NOWAIT` | có | có | không |
| `ON CONFLICT` | có | không | có |
| `ON DUPLICATE KEY UPDATE` | không | có | không |
| Insert nhiều dòng | có | có | có |
| Bọc ngoặc nhánh set op | có | có | không |

## 11.5. Giả lập hay từ chối

Quy tắc: **giả lập khi kết quả giống hệt, từ chối khi không**.

Được giả lập:

| Cấu trúc | Cách giả lập |
|---|---|
| `NULLS FIRST/LAST` | thêm một khóa sắp xếp `CASE WHEN ... IS NULL` đứng trước |
| `FILTER (WHERE ...)` | bọc biểu thức aggregate vào `CASE WHEN` |
| `BOOL_AND`/`BOOL_OR` | `MIN(...) <> 0`, `MAX(...) <> 0` |
| `OFFSET` không có `LIMIT` | thêm `LIMIT` giữ chỗ của engine |
| Cắt về đầu tháng, đầu tuần, ... | biểu thức ngày giờ tương đương |
| `day_of_week` | cộng bù để Chủ nhật luôn là 1 |

Bị từ chối:

| Cấu trúc | Lý do |
|---|---|
| `FULL JOIN` trên MySQL và SQLite | không có cách viết lại nào tương đương và rẻ |
| `DISTINCT ON` ngoài PostgreSQL | ngữ nghĩa "dòng đầu mỗi nhóm" không tái tạo được bằng `DISTINCT` |
| `STDDEV`, `VARIANCE`, `CORR` trên SQLite | giả lập bằng số học cho kết quả sai lệch |
| `WITHIN GROUP` ngoài PostgreSQL | trung vị là tính chất của cả nhóm đã sắp, không biểu thức trên một hàng nào dựng lại được |
| `FOR UPDATE` trên SQLite | không có khái niệm khóa dòng |
| `RETURNING` trên MySQL | không tồn tại |
| `DO NOTHING` trên MySQL | không có tương đương |
| `LPAD`/`RPAD` trên SQLite | không có hàm tương ứng |
| Sequence trên MySQL và SQLite | không có |

Từ chối trông như thế này:

```rust
match query.to_sql(&Sqlite) {
    Ok(rendered) => run(rendered),
    Err(err) => eprintln!("{err}"),
    // sqlite cannot express the operator StdDev
}
```

`RenderError` có bốn biến thể:

```rust
pub enum RenderError {
    UnsupportedOperator { dialect: &'static str, operator: Operator },
    UnsupportedFeature  { dialect: &'static str, feature: &'static str },
    MissingArgument     { operator: Operator, index: u8 },
    Invalid(&'static str),
}
```

`MissingArgument` báo lỗi trong một bảng template, không phải lỗi ở code người dùng.

## 11.6. Viết dialect riêng

`Dialect` là một trait bình thường, và mọi phương thức trừ năm cái đầu đều có bản mặc định:

```rust
pub trait Dialect {
    fn name(&self) -> &'static str;
    fn quote_ident(&self, ident: &str) -> String;
    fn placeholder(&self, index: usize) -> String;
    fn caps(&self) -> Caps;
    fn cast_type(&self, kind: CastKind) -> &'static str;

    fn template(&self, op: Operator) -> Option<Template> { /* bảng ANSI */ }
    fn precedence(&self, op: Operator) -> i16 { /* thang ANSI */ }
    fn bool_literal(&self, value: bool) -> &'static str { /* TRUE/FALSE */ }
    fn dummy_from(&self) -> Option<&'static str> { None }
    fn unlimited_limit(&self) -> Option<&'static str> { None }
}
```

Cách nhanh nhất để thêm một engine là bọc dialect gần nhất rồi override phần khác biệt.
Đó cũng chính là cách bộ test kiểm chứng các đường giả lập: một dialect chỉ khác SQLite ở chỗ `caps().aggregate_filter = false`, chạy trên database thật, để chứng minh đường giả lập đếm đúng những dòng mà đường native đếm.

## Bước tiếp theo

[Chương 12](./12-execution.md) chạy những câu SQL này trên database thật.

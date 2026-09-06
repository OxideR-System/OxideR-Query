---
id: type-safety
title: 14. Bảo đảm type-safety
sidebar_position: 14
---

# 14. Bảo đảm type-safety

Chương này liệt kê chính xác những gì trình biên dịch bắt được, kèm thông báo lỗi thật, và nói rõ những gì nó **không** bắt được.

Mọi lỗi dưới đây đều được kiểm chứng bằng doc test `compile_fail` trong chính crate, nên chúng không thể âm thầm ngừng hoạt động.

## 14.1. Sai kiểu ở toán hạng

```rust
User::name.eq(123);
```

```text
error[E0277]: the trait bound `{integer}: IntoExpr<String>` is not satisfied
```

Không có `Into` ngầm nào ở đây.
Danh sách nới rộng là tường minh (`&str` sang `String`, `i32` sang `i64`, ...), nên một phép so sánh chỉ biên dịch khi hai vế thực sự cùng miền giá trị.

## 14.2. Tham chiếu bảng chưa join

```rust
User::query().filter(Department::name.eq("AI"));
```

```text
error[E0277]: the trait bound `Nil: Contains<Department, _>` is not satisfied
```

Đây là bảo đảm mà QueryDSL không có.
Ở QueryDSL, quên một join sẽ cho một câu SQL sinh ra tích Descartes hoặc một lỗi từ database lúc chạy.

Cách hoạt động: `Select<S, F>` mang `S` là danh sách entity ở tầng type.
`filter` đòi `S: ContainsAll<S2, I>`, với `S2` là tập entity mà điều kiện tham chiếu tới.
Không tìm được chứng cứ thì không biên dịch.

Chi phí runtime bằng không: toàn bộ máy móc này biến mất sau khi biên dịch.

## 14.3. Toán tử không áp dụng cho kiểu

```rust
Order::status.sum();
```

```text
error[E0599]: the method `sum` exists for struct `Column<Order, String>`,
              but its trait bounds were not satisfied
note: the following trait bounds were not satisfied:
      `String: Numeric`
      which is required by `Column<Order, String>: NumericAggOps<String>`
```

```rust
User::active.gt(true);
```

```text
error[E0599]: the method `gt` exists for struct `Column<User, bool>`,
              but its trait bounds were not satisfied
```

`bool` không `Orderable`, nên `<`, `>`, `BETWEEN` và `ORDER BY` không xuất hiện trên cột boolean.

Đây là cách port lại cây kế thừa lớp của QueryDSL: thay vì `NumberExpression` kế thừa `ComparableExpression`, ở đây là trait bound trên `T`.
Hiệu quả giống nhau, nhưng không phải khai báo lại toán tử cho từng nhánh.

## 14.4. Cột của entity khác trong UPDATE

```rust
User::update().set(Department::name, "x");
```

```text
error[E0308]: mismatched types
   expected `Column<User, String>`, found `Column<Department, String>`
```

## 14.5. Subquery chọn sai kiểu

```rust
let sub = Department::query().scalar(Department::name);
User::query().filter(User::department_id.in_subquery(sub));
```

```text
error[E0308]: mismatched types
   expected `Subquery<_, i64>`, found `Subquery<Nil, String>`
```

`Subquery<F, T>` mang kiểu cột nó trả về, nên một phép `IN` chỉ biên dịch khi hai bên khớp kiểu.

## 14.6. Tuple giá trị không khớp tuple cột

```rust
User::insert().columns((User::name, User::age)).values(("ada", "36"));
```

```text
error[E0277]: the trait bound `&str: IntoExpr<i32>` is not satisfied
```

Số lượng cột cũng được kiểm tra: một tuple giá trị dài ngắn khác tuple cột không có impl nào khớp.

## 14.7. Field đã `skip` không tồn tại

```rust
#[derive(Entity)]
#[oxider(table = "people")]
struct Person {
    id: i64,
    #[oxider(skip)]
    display: String,
}

Person::query().select(Person::display);
```

Không có associated const nào tên `display`, nên đây là lỗi "no associated item named".

## 14.8. Subquery tương quan dùng sai chỗ

```rust
let item_total = OrderItem::query()
    .correlate::<Order>()
    .filter(OrderItem::order_id.eq(Order::id))
    .scalar(OrderItem::price.sum());

// `Order` không có trong query này:
User::query().select((User::name, item_total));
```

`item_total` mang `F = Cons<Order, Nil>`, và `select` đòi mọi entity trong `F` phải có trong phạm vi.
Đây chính là lý do `Select` có hai tham số kiểu; xem [mục 8.1](./08-subqueries.md).

## 14.9. Những gì KHÔNG được bảo đảm

Trung thực về giới hạn quan trọng ngang với việc liệt kê điểm mạnh.

**Nullability không nằm trong kiểu.**
`User::email` là `Column<User, String>` với cờ `nullable = true` ở runtime.
Trình biên dịch không nhắc bạn rằng một `LEFT JOIN` có thể sinh `NULL` ở cột vốn `NOT NULL`.
Lý do và đánh đổi ở [mục 2.4](./02-entities-and-columns.md).

**Mỗi entity chỉ một alias.**
`Aliased<E>` không mang tên alias vào kiểu, nên hai alias khác nhau của cùng bảng không phân biệt được ở tầng type.
Bản sao thứ ba phải dùng `col`, và mất kiểm tra phạm vi cho riêng nó.

**`col` và `raw` không được kiểm tra.**
Cả hai tồn tại để nói về những thứ hệ thống kiểu không biết, ví dụ CTE.
Kiểu trả về của chúng là do bạn khẳng định.

**Ràng buộc của SQL không được kiểm tra.**
Chọn một cột không nằm trong `GROUP BY`, dùng aggregate trong `WHERE`, hay `ORDER BY` một cột không có trong `SELECT DISTINCT` đều biên dịch được và sẽ bị database từ chối.
Mô hình hóa những luật đó ở tầng type sẽ khiến API nặng hơn nhiều so với giá trị nó mang lại.

**Schema có tồn tại hay không thì không biết.**
Entity mô tả những gì bạn khai báo, không phải những gì database thật sự có.
Muốn hai thứ khớp nhau thì [sinh entity từ schema](./13-codegen.md).

## 14.10. Kiểm tra ở đâu

| Bảo đảm | Kiểm tra bằng |
|---|---|
| Kiểu toán hạng | `IntoExpr<T>` |
| Phạm vi bảng | `Contains` / `ContainsAll` |
| Nhóm toán tử | `SqlType`, `Orderable`, `Numeric`, `Temporal` |
| Kiểu subquery | tham số `T` của `Subquery<F, T>` |
| Tương quan | tham số `F` của `Select<S, F>` |
| Cột thuộc entity nào | tham số `E` của `Column<E, T>` |
| Cấu trúc engine hỗ trợ | `Caps`, ở tầng render |

Ba mức, xảy ra theo thứ tự: sai kiểu thì không biên dịch, engine không hỗ trợ thì `to_sql` trả `Err`, còn lại thì database quyết định.

## Bước tiếp theo

[Chương 15](./15-api-cheatsheet.md) là bảng tra cứu nhanh toàn bộ API.

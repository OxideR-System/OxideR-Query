# 7. Bảo đảm type-safety

Điểm bán hàng cốt lõi của OxideR-Query là những lỗi dùng sai bị bắt ngay lúc biên dịch, với thông báo đọc được thay vì bức tường lỗi kiểu Diesel.
Chương này liệt kê các bất biến được thực thi và lỗi tương ứng.
Mỗi bất biến dưới đây đều được khóa bằng một `compile_fail` doctest trong crate facade, nên chúng không thể âm thầm hồi quy.

## 7.1. So sánh sai kiểu

Toán tử giá trị nhận `V: Into<T>` với `T` là kiểu cột.
So sánh với kiểu không đổi được sẽ hỏng biên dịch:

```rust
User::name.eq(123);
// error: the trait bound `i64: Into<String>` is not satisfied
```

## 7.2. Toán tử không hợp với kiểu cột

Toán tử là inherent method có điều kiện, nên gọi trên kiểu sai báo "method not found":

```rust
User::age.contains("50");
// error: no method named `contains` found for struct `Column<_, i32>`

User::flag.gt(true);
// error: the trait bound `bool: Orderable` is not satisfied
```

`contains`/`like`/`starts_with` chỉ có trên cột `String`.
`gt`/`ge`/`lt`/`le`/`asc`/`desc` chỉ có khi kiểu `Orderable`.

## 7.3. Tham chiếu bảng chưa join

Mỗi query mang phạm vi bảng ở tầng type.
Tham chiếu cột của bảng chưa vào phạm vi (chưa FROM/JOIN) là lỗi:

```rust
User::query().filter(Department::name.eq("AI"));
// error: the trait bound `Nil: Contains<Department, _>` is not satisfied
```

`filter`, `select`, `order_by`, `group_by`, `having` đều áp ràng buộc này.
`join`/`left_join` mở rộng phạm vi để cột bảng mới trở nên hợp lệ.

## 7.4. Khóa join lệch kiểu

`eq_column` đòi hai cột cùng kiểu Rust:

```rust
// Giả sử User::department_id: i64 và Department::code: String
User::department_id.eq_column(Department::code);
// error: mismatched types, expected `Column<_, i64>`, found `Column<_, String>`
```

## 7.5. Aggregate số học trên cột phi số

`sum`/`avg` đòi `T: Numeric`:

```rust
sum(Order::status); // status: String
// error: the trait bound `String: Numeric` is not satisfied
```

`min`/`max` đòi `T: Orderable` tương tự.

## 7.6. Mutation nhầm bảng

INSERT/UPDATE/DELETE chỉ trên một bảng.
Gán cột của entity khác là lỗi kiểu:

```rust
User::update().set(Department::name, "x");
// error: mismatched types (Column<Department, _> vào chỗ đợi Column<User, _>)
```

## 7.7. Subquery IN lệch kiểu

`in_subquery` đòi cột outer cùng kiểu với cột subquery chọn ra:

```rust
// department_id: i64 nhưng subquery chọn cột String
let sub = Department::query().scalar(Department::name);
User::query().filter(User::department_id.in_subquery(sub));
// error: mismatched types (Subquery<String> vào chỗ đợi Subquery<i64>)
```

## 7.8. Triết lý: type-safety ở nơi đáng, runtime ở nơi còn lại

Thư viện không type-state hóa mọi thứ.
Toán tử sống dưới dạng inherent method để lỗi hiện ra là "method not found" hoặc "trait bound not satisfied", dễ đọc hơn nhiều so với lỗi từ generic lồng sâu.
Một số bất biến vẫn để ở runtime khi ép vào type sẽ gây lỗi khó đọc mà lợi ích nhỏ:

- Nullability của cột hiện là cờ runtime trên `Column`, chưa phải tầng type (hoãn tới khi có tầng map row).
- UPDATE/DELETE không bắt buộc có `filter`; câu không filter là hợp lệ như SQL thuần.

Nắm rõ hai điểm này để không kỳ vọng nhầm về những gì compiler chặn.

## Bước tiếp theo

Xem [chương 8 - Tra cứu nhanh API](./08-api-cheatsheet.md).

---
id: security-model
title: 16. Mô hình bảo mật
sidebar_position: 16
---

# 16. Mô hình bảo mật

Chương này nói rõ thư viện chống được gì, không chống được gì, và ranh giới tin cậy nằm ở đâu.
Đọc trước khi đưa bất kỳ chuỗi nào do người dùng cuối kiểm soát vào một query.

## 16.1. Luật một câu

Giá trị luôn được bind, định danh luôn là hằng.

Mọi thứ bạn so sánh, gán hay chèn đều đi vào danh sách tham số và xuất hiện trong SQL dưới dạng placeholder.
Mọi thứ đặt tên cho một bảng, một cột, một alias hay một schema đều có kiểu `&'static str`, nên nó phải là literal trong mã nguồn hoặc do codegen sinh ra.

Hệ quả: đường đi tự nhiên của một giá trị không tin cậy là bind, và không có cách nào để nó trở thành định danh bằng một lần gọi API thông thường.

## 16.2. Giá trị

```rust
User::query().filter(User::name.eq(input_tu_nguoi_dung))
```

`input_tu_nguoi_dung` là `String` bất kỳ, kể cả `"'; DROP TABLE users; --"`.
Nó ra `WHERE "users"."name" = $1` và nằm trong `params`, không nằm trong chuỗi SQL.

Điều này đúng cho cả `LIKE`.
`contains`, `starts_with`, `ends_with` escape wildcard của toán hạng rồi mới bọc `%`, và sinh kèm `ESCAPE '!'`, nên tìm chuỗi `50%` không khớp `500 units`.

`LIMIT` và `OFFSET` là ngoại lệ có chủ ý: chúng nhận `u64` và được viết thẳng vào SQL.
Một số nguyên không dấu không biểu diễn được ký tự nào, nên không có gì để chèn.

## 16.3. Định danh

`&'static str` là hàng rào, không phải quy ước.
Trình biên dịch từ chối một `String` dựng lúc chạy ở mọi vị trí định danh:

```rust
let ten_bang = format!("t_{}", input);
select_from_name(&ten_bang);
// error[E0597]: `ten_bang` does not live long enough
```

Có đúng một cách lách: `String::leak` hoặc `Box::leak` biến `String` lúc chạy thành `&'static str`.
Đó là lỗ hổng bạn tự mở, và nó cũng rò bộ nhớ.
Nếu ứng dụng thật sự cần tên bảng động, hãy so khớp chuỗi đầu vào với một danh sách trắng gồm các literal và dùng literal đó:

```rust
let bang: &'static str = match input {
    "users" => "users",
    "posts" => "posts",
    _ => return Err(...),
};
```

Renderer vẫn quote định danh theo dialect (`"..."` hoặc `` `...` ``) và nhân đôi ký tự quote bên trong, nên một tên hợp lệ chứa dấu quote không phá được cú pháp.
Đó là lớp phòng thủ thứ hai, không phải lớp thứ nhất.

## 16.4. `raw` và `col`

`raw` và `RawBuilder` chèn SQL nguyên văn.
Cả hai nhận `&'static str`, nên đầu vào không tin cậy không tới được đó, nhưng chúng là cửa duy nhất bỏ qua toàn bộ tầng template.
Coi nội dung của chúng như mã nguồn: chỉ viết literal, và bind mọi giá trị bằng `.bind(...)` của `RawBuilder` thay vì nội suy vào chuỗi.

`col(qualifier, name)` cũng vậy: cả hai đối số là `&'static str` và không được kiểm tra phạm vi.

## 16.5. Tham số đặt tên

`param("ten")` dựng một chỗ trống, `bind("ten", value)` điền vào nó.

```rust
let mau = User::query().filter(User::age.ge(param::<i32>("tuoi_toi_thieu")));

mau.clone().bind("tuoi_toi_thieu", 18).to_sql(&Postgres)?;
mau.bind("tuoi_toi_thieu", 21).to_sql(&Postgres)?;
```

Một tên chưa bind làm `to_sql` trả `Err(RenderError::UnboundParameter)`, chứ không sinh ra query thiếu điều kiện.
Bind cùng một tên hai lần thì giá trị sau thắng.
Bind áp dụng cho cả câu lệnh, nên tham số nằm trong subquery vẫn được điền từ câu lệnh ngoài cùng.

## 16.6. Query động và độ sâu

Một vòng lặp `filter` trên danh sách đầu vào là mẫu bình thường, và các mệnh đề `AND`/`OR` liên tiếp được làm phẳng thành một node nhiều toán hạng thay vì lồng nhau.
Nghĩa là một nghìn điều kiện `AND` vẫn là cây sâu một tầng.

Với cây thật sự sâu, ví dụ subquery lồng subquery do một API sinh ra, renderer dừng ở `MAX_DEPTH` (256) và trả `Err(RenderError::TooDeep)` thay vì tràn stack.
Giới hạn này là hằng công khai, đọc được qua `oxider_query::MAX_DEPTH`.

## 16.7. Codegen

`oxider-query-codegen` đọc tên bảng và tên cột từ database rồi sinh mã Rust.
Tên bảng đi vào truy vấn introspection dưới dạng tham số bind, không phải nội suy chuỗi.
Tên bảng và tên cột đi vào mã sinh ra thì được escape theo luật string literal của Rust, nên một tên chứa dấu nháy kép hay ký tự điều khiển không thoát ra ngoài literal để trở thành mã.

Điều đó biến một schema thù địch từ vấn đề thực thi mã thành vấn đề đặt tên xấu.
Dù vậy, codegen là công cụ lúc build: chỉ chĩa nó vào database bạn tin.

## 16.8. Thực thi

`oxider-query-exec` tách `Error::Render` khỏi `Error::Database`, nên một câu lệnh engine không diễn đạt được sẽ hỏng trước khi chạm tới connection.

`db.transaction(async |tx| { ... })` commit khi closure trả `Ok` và rollback khi trả `Err`.
Nếu rollback thất bại, lỗi của closure vẫn là lỗi được trả về, vì đó mới là nguyên nhân.

## 16.9. Bảng tổng kết

| Đầu vào | Đi đâu | Ai chịu trách nhiệm |
|---|---|---|
| Giá trị so sánh, gán, chèn | danh sách bind | thư viện |
| Toán hạng `LIKE` | bind, đã escape wildcard | thư viện |
| `LIMIT` / `OFFSET` | nội suy, kiểu `u64` | thư viện |
| Tên bảng, cột, alias, schema | nội suy, đã quote | bạn, qua `&'static str` |
| `raw`, `col` | nội suy nguyên văn | bạn |
| Tên bảng, cột từ codegen | mã sinh ra, đã escape | thư viện, nếu schema đáng tin |

## Bước tiếp theo

[Chương 14](./14-type-safety.md) liệt kê những gì trình biên dịch bắt được, và [chương 11](./11-dialects.md) liệt kê những gì mỗi engine từ chối.

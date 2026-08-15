# 3. SELECT và lọc dữ liệu

Chương này bao trọn phần dựng câu SELECT một bảng: chọn cột, lọc, ghép điều kiện, sắp xếp, phân trang và query động.
JOIN nhiều bảng ở [chương 4](./04-joins.md).

## 3.1. Vòng đời một query

```rust
User::query()        // Select<Cons<User, Nil>>  -- FROM "users"
    .select(...)     // chọn cột (mặc định là tất cả)
    .filter(...)     // WHERE
    .group_by(...)   // GROUP BY   (chương 5)
    .having(...)     // HAVING     (chương 5)
    .order_by(...)   // ORDER BY
    .limit(n)        // LIMIT
    .offset(n)       // OFFSET
    .render(&dialect) // -> Rendered { sql, params }
```

Mọi phương thức nhận `self` và trả về builder mới, nên nối chuỗi thoải mái.
Ngoài `render`, còn `build()` trả về AST `SelectQuery` nếu bạn muốn tự xử lý.

### Về thứ tự đọc so với SQL

Query bắt đầu từ `User::query()` (tức FROM), khác với SQL viết `SELECT` trước.
Đây là ràng buộc cần thiết cho type-safety: phải có bảng trong scope trước thì `User::id` mới tồn tại để compiler kiểm tra và gợi ý.
Cùng lý do đó, QueryDSL (Java), LINQ (C#) và Diesel (Rust) đều đặt nguồn dữ liệu trước.
Ngoài ra thứ tự `from -> where -> select` khớp đúng thứ tự SQL *xử lý* thật (FROM chạy trước SELECT), chỉ khác thứ tự SQL *viết ra*.

Builder không ép thứ tự các mệnh đề còn lại, nên nếu muốn đọc gần SQL hơn, cứ đặt `select` ngay sau `query`:

```rust
User::query().select((User::id, User::name)).filter(User::age.ge(18));
//  từ users,      chọn id, name,             lọc age >= 18
```

`filter` mang tên vậy vì `where` là từ khóa Rust, không đặt được làm tên method; `filter` cũng là quy ước idiomatic giống iterator.

## 3.2. Chọn cột (projection)

Không gọi `select` thì projection mặc định là `SELECT *`.

Chọn một cột hoặc một tuple từ 2 tới 6 phần tử:

```rust
User::query().select(User::id);                     // 1 cột
User::query().select((User::id, User::name));       // 2 cột
User::query().select((User::id, User::name, User::age)); // 3 cột, tối đa 6
```

Phần tử projection có thể là cột hoặc aggregate (xem [chương 5](./05-aggregates-mutations-subqueries.md)), miễn là mọi bảng nó tham chiếu đều đang trong phạm vi query.

## 3.3. Toán tử so sánh trên cột

Tất cả toán tử là inherent method trên `Column`, nên gõ sai kiểu báo lỗi "method not found" hoặc "trait bound not satisfied" dễ đọc.

| Nhóm | Phương thức | Điều kiện kiểu | SQL |
|------|-------------|----------------|-----|
| Bằng | `eq(v)`, `ne(v)` | mọi cột (`T: ToSqlValue`) | `=`, `<>` |
| Thứ tự | `gt(v)`, `ge(v)`, `lt(v)`, `le(v)` | `T: Orderable` | `>`, `>=`, `<`, `<=` |
| Chuỗi | `like(p)`, `contains(s)`, `starts_with(p)` | chỉ cột `String` | `LIKE` |
| Khóa join | `eq_column(other)` | hai cột cùng `T` | `=` (chương 4) |
| Subquery | `in_subquery(sub)`, `not_in_subquery(sub)` | subquery cùng `T` | `IN` / `NOT IN` (chương 5) |

Mọi toán tử giá trị nhận `V: Into<T>`, nên `User::name.eq("Alice")` hợp lệ dù cột là `String`.

```rust
User::query().filter(User::age.gt(21));
// WHERE ("users"."age" > $1)   params: [21]

User::query().filter(User::name.starts_with("Ng"));
// WHERE ("users"."name" LIKE $1)   params: ["Ng%"]
```

`contains` bọc chuỗi thành `%s%`, `starts_with` thành `s%`.
Lưu ý: các ký tự đặc biệt của LIKE (`%`, `_`) trong tham số chưa được escape; đây là bước gia cố text-ops dự kiến sau.

## 3.4. Ghép điều kiện với `and` / `or`

`Predicate` có `and` và `or`, mỗi cái nhận một predicate khác và gộp phạm vi bảng của cả hai:

```rust
User::query()
    .filter(User::age.ge(18).and(User::name.contains("nguyen")))
    .render(&Postgres);
// WHERE (("users"."age" >= $1) AND ("users"."name" LIKE $2))

User::query()
    .filter(User::age.lt(13).or(User::age.gt(65)))
    .render(&Postgres);
// WHERE (("users"."age" < $1) OR ("users"."age" > $2))
```

Thứ tự param trong `params` khớp thứ tự xuất hiện trong SQL.

Gọi `filter` nhiều lần cũng được; các lời gọi nối với nhau bằng `AND`:

```rust
User::query()
    .filter(User::age.ge(18))
    .filter(User::name.contains("le"));
// WHERE ("users"."age" >= $1) AND ("users"."name" LIKE $2)
```

## 3.5. Sắp xếp

Gọi `asc()` hoặc `desc()` trên cột (cột phải `Orderable`) rồi đưa vào `order_by`:

```rust
User::query().order_by(User::age.desc());
// ORDER BY "users"."age" DESC
```

Cần sắp theo nhiều cột thì gọi `order_by` nhiều lần theo thứ tự ưu tiên:

```rust
User::query()
    .order_by(User::age.desc())
    .order_by(User::name.asc());
// ORDER BY "users"."age" DESC, "users"."name" ASC
```

## 3.6. Phân trang

```rust
User::query().limit(20).offset(40);
// LIMIT 20 OFFSET 40
```

`limit` và `offset` nhận `u64`.

## 3.7. Query động với `filter_opt`

`filter_opt` nhận `Option<Predicate<_>>`.
`Some` thì thêm điều kiện, `None` thì bỏ qua.
Rất tiện để dựng bộ lọc từ input tùy chọn mà không cần nối chuỗi if/else:

```rust
struct UserFilter {
    name: Option<String>,
    min_age: Option<i32>,
}

fn search(f: UserFilter) -> Rendered {
    User::query()
        .filter_opt(f.name.map(|v| User::name.contains(v)))
        .filter_opt(f.min_age.map(|v| User::age.ge(v)))
        .render(&Postgres)
}
```

Nếu cả hai `None`, câu SQL không có mệnh đề `WHERE`.

## 3.8. Lấy AST thay vì SQL

`build()` trả về `SelectQuery`, tức AST độc lập dialect.
Dùng khi bạn muốn thanh tra, cache, hoặc render nhiều dialect từ cùng một AST:

```rust
let ast = User::query().filter(User::age.ge(18)).build();
let pg = ast.render(&Postgres);
let my = ast.render(&MySql);
```

## Bước tiếp theo

- Ghép nhiều bảng: [chương 4 - JOIN](./04-joins.md).
- Thống kê, GROUP BY, mutation, subquery: [chương 5](./05-aggregates-mutations-subqueries.md).

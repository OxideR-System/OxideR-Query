---
id: dml
title: 10. INSERT, UPDATE, DELETE
sidebar_position: 10
---

# 10. INSERT, UPDATE, DELETE

Ba builder này dùng cùng một tầng render với `Select`, nên mọi biểu thức đã học đều dùng lại được ở đây.

## 10.1. INSERT một dòng

```rust
User::insert()
    .set(User::name, "ada")
    .set(User::age, 36)
    .set(User::active, true);
```

```sql
-- PostgreSQL
INSERT INTO "users" ("name", "age", "active") VALUES ($1, $2, $3)
-- MySQL
INSERT INTO `users` (`name`, `age`, `active`) VALUES (?, ?, ?)
```

Cột phải thuộc đúng entity đang insert.
`User::insert().set(Department::name, "x")` là lỗi biên dịch.

Schema được giữ nguyên:

```rust
AuditLog::insert().set(AuditLog::actor_name, "ada").set(AuditLog::action, "login");
// INSERT INTO "ops"."audit_log" ("actor", "action") VALUES ($1, $2)
```

## 10.2. INSERT nhiều dòng

Khai báo danh sách cột một lần bằng `columns`, rồi nối từng dòng bằng `values`:

```rust
User::insert()
    .columns((User::name, User::age))
    .values(("ada", 36))
    .values(("grace", 45));
```

```sql
INSERT INTO "users" ("name", "age") VALUES ($1, $2), ($3, $4)
```

Tuple giá trị phải khớp tuple cột cả về số lượng lẫn kiểu.
`values(("ada", "36"))` không biên dịch được.

`columns` đổi kiểu của builder từ `Insert<E, ()>` sang `Insert<E, Cs::Types>`, nên `set` và `columns` loại trừ lẫn nhau: một câu insert hoặc đặt từng cột, hoặc khai báo danh sách cột rồi nạp dòng, không trộn hai kiểu.

Insert nhiều dòng trong một câu cần `Caps::multi_row_insert`; cả ba engine đều có.

## 10.3. INSERT lấy dữ liệu từ SELECT

```rust
Post::insert()
    .columns((Post::user_id, Post::title))
    .from_query(
        User::query()
            .filter(User::active.eq(true))
            .select((User::id, User::name)),
    );
```

```sql
INSERT INTO "posts" ("user_id", "title")
SELECT "users"."id", "users"."name" FROM "users" WHERE "users"."active" = $1
```

## 10.4. Upsert

```rust
User::insert()
    .set(User::email, "ada@example.com")
    .set(User::name, "ada")
    .on_conflict(["email"])
    .do_update()
    .set(User::name, excluded(User::name))
    .end();
```

```sql
-- PostgreSQL và SQLite
INSERT INTO "users" ("email", "name") VALUES ($1, $2)
ON CONFLICT ("email") DO UPDATE SET "name" = excluded."name"

-- MySQL diễn đạt cùng ý bằng mệnh đề của riêng nó
INSERT INTO `users` (`email`, `name`) VALUES (?, ?)
ON DUPLICATE KEY UPDATE `name` = VALUES(`name`)
```

`excluded(column)` trỏ tới dòng vừa bị từ chối.
Nó render thành `excluded."col"` trên engine có `ON CONFLICT`, và `VALUES(\`col\`)` trên MySQL.

`do_nothing()` không có tương đương trên MySQL nên bị từ chối ở đó:

```rust
let stmt = User::insert().set(User::email, "ada@example.com")
    .on_conflict(["email"])
    .do_nothing();

stmt.to_sql(&Postgres).unwrap();   // ... ON CONFLICT ("email") DO NOTHING
stmt.to_sql(&MySql).unwrap_err();  // feature: "DO NOTHING"
```

## 10.5. RETURNING

```rust
User::insert().set(User::name, "ada").returning((User::id, User::created_at));
```

```sql
-- PostgreSQL và SQLite
INSERT INTO "users" ("name") VALUES ($1) RETURNING "users"."id", "users"."created_at"
-- MySQL: bị từ chối, feature "RETURNING"
```

`returning` có trên cả `Insert`, `Update` và `Delete`.

## 10.6. UPDATE

```rust
Post::update()
    .set(Post::views, Post::views.add(1))
    .filter(Post::id.eq(42));
// UPDATE "posts" SET "views" = "posts"."views" + $1 WHERE "posts"."id" = $2
```

Vế phải là một biểu thức đầy đủ, nên nó đọc được chính cột đang ghi.

Nhiều cột được đặt theo đúng thứ tự gọi, và `set_null` đặt `NULL` tường minh:

```rust
User::update()
    .set(User::name, "grace")
    .set(User::active, false)
    .set_null(User::email)
    .filter(User::id.eq(1));
// UPDATE "users" SET "name" = $1, "active" = $2, "email" = NULL WHERE "users"."id" = $3
```

`set_null` không giới hạn ở cột mà metamodel đánh dấu nullable: cờ nullable phản ánh schema như nó được khai báo, còn câu lệnh có hợp lệ hay không là việc của database.

### UPDATE đọc bảng khác

```rust
User::update()
    .from(Department::table())
    .set(User::name, Department::name)
    .filter(User::department_id.eq(Department::id));
```

```sql
UPDATE "users" SET "name" = "departments"."name" FROM "departments"
WHERE "users"."department_id" = "departments"."id"
```

`from` mở rộng phạm vi của `Update`, đúng như `inner_join` làm với `Select`.

## 10.7. DELETE

```rust
Post::delete();
// DELETE FROM "posts"
```

Không có điều kiện thì xóa mọi dòng.
Thư viện không chặn việc này, nên hãy chắc chắn đó là ý bạn muốn.

```rust
Post::delete().filter(Post::views.eq(0)).returning(Post::id);
// DELETE FROM "posts" WHERE "posts"."views" = $1 RETURNING "posts"."id"
```

### DELETE dùng bảng khác

```rust
Post::delete()
    .using(User::table())
    .filter(Post::user_id.eq(User::id).and(User::active.eq(false)));
```

```sql
DELETE FROM "posts" USING "users"
WHERE "posts"."user_id" = "users"."id" AND "users"."active" = $1
```

### DELETE theo subquery

Không cần đưa bảng thứ hai vào phạm vi nếu điều kiện đi qua một subquery:

```rust
let inactive = User::query().filter(User::active.eq(false)).scalar(User::id);
Post::delete().filter(Post::user_id.in_subquery(inactive));
```

```sql
DELETE FROM "posts"
WHERE "posts"."user_id" IN (SELECT "users"."id" FROM "users" WHERE "users"."active" = $1)
```

## 10.8. Điều kiện tùy chọn

`Update` và `Delete` cũng có `filter_opt`, hoạt động giống hệt như trên `Select`.

## Bước tiếp theo

[Chương 11](./11-dialects.md) giải thích vì sao cùng một câu lệnh lại ra SQL khác nhau, và khi nào một dialect từ chối.

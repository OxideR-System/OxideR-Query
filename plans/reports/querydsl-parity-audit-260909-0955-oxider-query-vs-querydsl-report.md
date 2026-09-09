# OxideR-Query vs QueryDSL: đánh giá mức độ hoàn thiện

Ngày: 2026-09-09
Phiên bản đánh giá: OxideR-Query v0.2.2
Nguồn đối chiếu: `../querydsl` (fork OpenFeign), bản có trên máy.

## Kết luận ngắn

Chưa port 100%, và không nên đặt mục tiêu 100%.

Nếu chỉ tính phần "SQL query builder" (tương đương `querydsl-core` + `querydsl-sql`) trên ba dialect Postgres/MySQL/SQLite: khoảng **90% hoàn chỉnh**.
Nếu tính toàn bộ QueryDSL (JPA, Collections, MongoDB, R2DBC, Scala, Kotlin, Spatial, JSON, Spring, APT): khoảng **35-40%**, vì phần lớn các module đó nằm ngoài phạm vi đã chọn.

Phần lõi (AST, catalog toán tử, template engine, renderer, tầng typed) bám rất sát kiến trúc QueryDSL và ở vài chỗ vượt hơn.
Phần còn thiếu tập trung ở ngoại vi: projection/mapping, execution, codegen, số lượng dialect.

## 1. Catalog toán tử

| | QueryDSL | OxideR-Query |
|---|---|---|
| Định danh trong enum | ~171 (`Ops` + 5 enum lồng) + ~57 (`SQLOps`) | 164 variant |
| Render được | toàn bộ theo dialect | 161/164 |

Phần chênh tên phần lớn do đặt tên hoặc biến thể arity: `SUBSTR_1ARG`/`SUBSTR_2ARGS`, `LPAD`/`LPAD2`, `ROUND`/`ROUND2`, `GOE` thành `ge`, `LOE` thành `le`, `MULT` thành `mul`, `SUM_AGG` thành `sum`.
Nhóm bỏ có chủ đích: toán tử của backend JPA và collection trong bộ nhớ (`INSTANCE_OF`, `COL_SIZE`, `MAP_IS_EMPTY`, `CONTAINS_KEY`, `LIST`, `SET`, `SINGLETON`, `ORDINAL`, `WRAPPED`, `ALIAS`).
Bỏ nhóm này là đúng: không backend nào trong OxideR cần chúng.

Thiếu thật sự, đều thuộc SQL analytic:

- Họ hồi quy `REGR_SLOPE`, `REGR_INTERCEPT`, `REGR_COUNT`, `REGR_R2`, `REGR_AVGX`, `REGR_AVGY`, `REGR_SXX`, `REGR_SYY`, `REGR_SXY`: 9 hàm, không có.
- `LISTAGG`: không có. `GROUP_CONCAT` có nhưng khác cú pháp và khác dialect.
- `PERCENTILE_CONT`, `PERCENTILE_DISC`, `RATIO_TO_REPORT`: **đã khai báo variant nhưng không có template ở bất kỳ dialect nào**, gọi tới sẽ trả `Err`. Lý do ghi trong `crates/oxider-query-core/src/dialect/templates/agg.rs:52` - builder chưa diễn đạt được dạng `WITHIN GROUP (ORDER BY ...)`.
- `QUALIFY`, `TABLESAMPLE`, `IGNORE NULLS`/`RESPECT NULLS` trên window function: không có.

Đánh giá: **~95%** phần toán tử SQL có ý nghĩa với 3 dialect.
Ba variant khai báo mà không render được là điểm nợ nhìn thấy rõ nhất.

## 2. Bề mặt SELECT

Gần đầy đủ so với `SQLCommonQuery` + `ProjectableSQLQuery`.

Có: mọi kiểu join (inner/left/right/full/cross), join subquery và join theo tên, alias và self-join ở tầng type, `DISTINCT` và `DISTINCT ON`, `GROUP BY`/`HAVING`, window function kèm `WINDOW` clause đặt tên, `ORDER BY` có nulls ordering, `LIMIT`/`OFFSET`/`page`, row locking đầy đủ (`FOR UPDATE`/`SHARE`/`NO KEY UPDATE`/`KEY SHARE`, `NOWAIT`, `SKIP LOCKED`), set operation, CTE kể cả đệ quy, subquery tương quan có kiểm tra biến tự do ở tầng type.

Thiếu:

- **`LATERAL` join**: không có.
- **`addFlag`/`addJoinFlag`**: cơ chế chèn SQL thô vào một vị trí cú pháp xác định của QueryDSL. OxideR có `raw`/`col` nhưng không có khái niệm vị trí (`Position.AFTER_GROUP_BY`...). Đây là cách QueryDSL làm `WITH ROLLUP` của MySQL và table hint của SQL Server.
- **Table-valued function trong `FROM`** (`RelationalFunctionCall`): không có.
- `WITHIN GROUP`: đã nói ở trên.

Công bằng mà nói: `GROUPING SETS`/`ROLLUP`/`CUBE` thì QueryDSL cũng không có dạng first-class, chỉ qua flag. Không tính là thiếu so với QueryDSL.

Đánh giá: **~92%.**

## 3. DML

Có: insert nhiều dòng, insert-select, upsert (`ON CONFLICT ... DO UPDATE`/`DO NOTHING`), `RETURNING`, `UPDATE ... FROM`, `DELETE ... USING`.

Thiếu:

- **`MERGE`**: bỏ có chủ đích, đã ghi trong plan (chỉ Oracle/SQL Server). Chấp nhận được.
- **Batch** (`SQLInsertBatch`, `SQLUpdateBatch`, `addBatch`/`executeBatch`): không có. Thiếu sót thật, ảnh hưởng throughput khi ghi hàng loạt.
- **DDL** (`CreateTableClause`, `DropTableClause`, `ForeignKeyBuilder`, `IndexData`): không có.
- **`REPLACE` của MySQL**: không có.

Đánh giá: **~85%.**

## 4. Projection và mapping - khoảng trống lớn nhất

QueryDSL có: `Projections.bean/fields/constructor/tuple/map/list/array`, `QBean`, `ConstructorExpression`, `MappingProjection`, và `transform(groupBy(...))` để gấp kết quả phẳng thành cây.

OxideR có: tuple projection arity 2..=12. Hết.

Thiếu:

- `#[derive(Projection)]`: nằm trong plan Phase 5, chưa làm.
- **GroupBy transformer**: chưa làm. Đây là tính năng QueryDSL được dùng nhiều thứ hai sau bản thân builder, vì nó giải bài toán one-to-many mà không cần ORM.
- Mapping động (`Mapper`, `BeanMapper`, `AnnotationMapper`) để insert/update từ một object có sẵn.

Đánh giá: **~30%.** Khoảng cách rõ nhất trong toàn bộ bản port.

## 5. Execution

| | QueryDSL | OxideR-Query |
|---|---|---|
| Backend | mọi JDBC driver | sqlx Postgres + SQLite |
| API kết quả | `fetch`, `fetchOne`, `fetchFirst`, `fetchCount`, `fetchResults`, `iterate`, `stream` | `fetch_all`, `fetch_one`, `fetch_optional`, `execute` |
| Transaction | qua Spring/JDBC | `transaction(closure)` + `begin()` thủ công |
| Batch | có | không |
| Listener/hook | `SQLListener`, `SQLDetailedListener` | không |
| `StatementOptions` (fetchSize, timeout, maxRows) | có | không |
| Dịch lỗi | `SQLExceptionTranslator` | chỉ tách `Render` và `Database` |

Thiếu đáng kể:

- **MySQL backend**: README nói builder hỗ trợ MySQL nhưng `Db` thì không, người dùng phải tự bind. Bất đối xứng này hiện đang phải giải thích bằng một đoạn cảnh báo ngay đầu README.
- **`fetch_count`/`fetch_results`**: không có cách lấy tổng số dòng cho phân trang mà không tự viết query đếm.
- **Streaming**: không trả `Stream`, tập kết quả lớn phải nạp hết vào bộ nhớ.

Đánh giá: **~55%.**

## 6. Codegen

QueryDSL có hai đường: APT sinh Q-type từ class Java/JPA entity, và `MetaDataExporter` introspect bất kỳ database JDBC nào, kèm PK, FK, index, schema, naming strategy, `namemapping`.

OxideR có `#[derive(Entity)]` (tương đương APT, đủ tốt) và `oxider-query-codegen` **chỉ cho SQLite**, không sinh PK/FK/index, không có naming strategy cấu hình được.

Thiếu: introspect Postgres và MySQL (Phase 8, chưa làm), quan hệ khóa ngoại, chiến lược đặt tên.

Đánh giá: **~40%.**

## 7. Dialect

QueryDSL: CUBRID, DB2, Derby, Firebird, H2, HSQLDB, MySQL, Oracle, PostgreSQL, SQLite, SQL Server 2005/2008/2012, Teradata, Turso - khoảng 15.
OxideR: 3.

Đây là lựa chọn phạm vi, không phải lỗi. Kiến trúc template khiến thêm dialect chỉ là override vài dòng, chi phí mở rộng thấp.
Nhưng nếu câu hỏi là "đã port 100% chưa" thì con số là **3/15**.

## 8. Hệ thống kiểu giá trị

QueryDSL có khoảng 40 `Type` impl: `BigDecimal`, `UUID`, `Blob`/`Clob`, `ArrayType`, JSR-310 đầy đủ, enum theo tên hoặc ordinal, JSON, spatial.

OxideR `Value` có 9 biến thể: `Null`, `Bool`, `Int(i64)`, `Real(f64)`, `Text`, `Bytes`, `Date`, `Time`, `DateTime`.
`ToSqlValue` phủ i8..i64, u8..u32, f32/f64, bool, `String`, `Vec<u8>`, cộng chrono sau feature flag.

Thiếu: `u64`/`i128`, decimal chính xác (`rust_decimal` hoặc `bigdecimal`), `UUID`, JSON/JSONB, mảng Postgres, enum.
Với Postgres đây là hạn chế thực tế: rất nhiều schema production dùng `uuid`, `numeric` và `jsonb`.

Đánh giá: **~45%.**

## 9. Module QueryDSL không port, có chủ đích

`querydsl-jpa`, `querydsl-collections`, `querydsl-mongodb`, `querydsl-r2dbc`, `querydsl-scala`, `querydsl-kotlin`, `querydsl-spatial`, `querydsl-sql-json`, `querydsl-sql-spring`, `querydsl-guava`.

Định vị đã chốt là "không làm ORM, không tự viết driver", nên bỏ các module này là nhất quán.
Ngoại lệ đáng cân nhắc: `querydsl-collections` (query trên collection trong bộ nhớ) sẽ rất tự nhiên trong Rust và dùng lại được toàn bộ tầng typed.

## 10. Chỗ OxideR làm tốt hơn QueryDSL

Ghi để cân bằng đánh giá, không phải để tự khen:

1. **Kiểm tra phạm vi bảng ở tầng type.** `filter`/`select`/`order_by` chỉ nhận cột của bảng thực sự có trong query. QueryDSL không có.
2. **`to_sql` trả `Result`.** Dialect không diễn đạt được thì từ chối tại chỗ, thay vì sinh SQL sai để database từ chối sau. QueryDSL im lặng render fallback ANSI - chính cơ chế này đã bắt được bug `STDDEV` trên SQLite.
3. **`LIKE` escape cả toán hạng động.** QueryDSL chỉ escape hằng và bỏ qua giá trị động, nên tìm `50%` sẽ khớp `500 units`.
4. **Định danh là `&'static str`.** SQL injection qua tên bảng/cột không biên dịch được, không phải dựa vào quoting.
5. **Kiểm tra kiểu của toán tử qua trait bound.** `Order::status.sum()` không biên dịch.

## Bảng tổng hợp

| Vùng | Mức hoàn thiện | Ghi chú |
|---|---|---|
| Catalog toán tử | ~95% | thiếu regr*, listagg; 3 variant khai báo mà không render được |
| Bề mặt SELECT | ~92% | thiếu LATERAL, flag theo vị trí, table function |
| DML | ~85% | thiếu batch, DDL; MERGE bỏ có chủ đích |
| Projection và mapping | ~30% | chỉ tuple; thiếu derive và GroupBy transformer |
| Execution | ~55% | thiếu MySQL, count/paging, streaming, batch |
| Codegen | ~40% | chỉ SQLite, không PK/FK/index |
| Dialect | 3/15 | lựa chọn phạm vi |
| Kiểu giá trị | ~45% | thiếu uuid, decimal, json, array |
| Backend ngoài SQL | 0% | ngoài phạm vi, hợp lý |
| **Tổng, phạm vi "SQL builder 3 dialect"** | **~90%** | |
| **Tổng, so toàn bộ QueryDSL** | **~35-40%** | |

## Thứ tự ưu tiên đề xuất

Xếp theo giá trị trên công sức, không theo thứ tự trong plan:

1. **GroupBy transformer + `#[derive(Projection)]`** (phần còn lại của Phase 5). Khoảng trống lớn nhất, và là thứ khiến thư viện dùng được cho ứng dụng thật chứ không chỉ sinh chuỗi SQL.
2. **Backend MySQL cho exec** (Phase 7). Xoá bất đối xứng "builder hỗ trợ 3, exec hỗ trợ 2" mà README đang phải giải thích dài dòng.
3. **`fetch_count` và kiểu kết quả phân trang.** Phân trang là nhu cầu phổ thông; hiện người dùng phải tự viết query đếm.
4. **Kiểu giá trị `uuid`, decimal, `json`**, sau feature flag, giống cách `chrono` đang làm. Không có ba kiểu này thì nhiều schema Postgres thật không dùng được.
5. **Dọn 3 variant không render được.** Hoặc thêm `WITHIN GROUP` vào builder, hoặc gỡ variant. Enum công khai hứa một thứ mà mọi dialect đều từ chối là nợ API.
6. **Codegen Postgres** (Phase 8). Giá trị cao nhưng công sức cũng cao nhất trong danh sách này.
7. **Batch DML và streaming.** Chỉ cần khi có người dùng chạm trần hiệu năng.
8. **CI workflow** (phần còn lại của Phase 10). `make check` đã có, chỉ còn nối vào GitHub Actions.

Cố tình xếp thấp: `LATERAL`, table function, DDL, thêm dialect. Đều là bề rộng chứ không phải chiều sâu, và kiến trúc template khiến chúng rẻ khi nào thực sự cần.

## Ghi chú về độ chính xác của tài liệu hiện có

`plans/260905-2058-querydsl-full-port/plan.md` đã lệch thực tế: bảng trạng thái ghi Phase 7 "CHƯA", nhưng backend Postgres đã có từ commit `fa4b8ef` và đang chạy trong v0.2.0 trở lên.
Nên cập nhật plan, nếu không lần đánh giá sau lại đọc nhầm.

## Câu hỏi chưa giải quyết

1. Mục tiêu thật sự là parity với QueryDSL, hay là "query builder tốt nhất cho Rust"? Nếu là vế sau thì `regr*`, `LISTAGG`, DDL và 12 dialect còn lại nên gỡ hẳn khỏi roadmap thay vì để treo.
2. `PERCENTILE_CONT`/`PERCENTILE_DISC`/`RATIO_TO_REPORT`: thêm `WITHIN GROUP` vào builder hay gỡ variant?
3. Có mở hướng tương đương `querydsl-collections` (query trên `Vec<T>` trong bộ nhớ, dùng lại tầng typed) không?
4. Kiểu decimal: chọn `rust_decimal` hay `bigdecimal`, hay cho cả hai sau feature flag như `chrono`?

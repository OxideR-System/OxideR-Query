# OxideR-Query: roadmap sau khi đổi mục tiêu sang "query builder tốt nhất cho Rust"

Status: ĐANG CHẠY. P1-P5 XONG (2026-09-09). Kế tiếp: P6.
Ngày tạo: 2026-09-09
Baseline: v0.2.2, 157 scenario test + exec/codegen test, clippy sạch, fmt sạch.
Thay thế phần "Việc còn lại" của `plans/260905-2058-querydsl-full-port/plan.md`.

## Quyết định nền

Mục tiêu **không còn là** parity với QueryDSL.
Mục tiêu là query builder tốt nhất cho Rust trên Postgres/MySQL/SQLite.

Hệ quả trực tiếp: mọi hạng mục chỉ tồn tại vì "QueryDSL có" mà không có người dùng Rust nào cần thì gỡ khỏi roadmap, không để treo.
QueryDSL vẫn là nguồn tham chiếu kiến trúc (Operator + Template + precedence ladder), không còn là thước đo hoàn thành.

## Phi mục tiêu, chốt rõ để khỏi bàn lại

| Bỏ | Lý do |
|---|---|
| Họ `REGR_*` (9 hàm) | thống kê hồi quy trong SQL, gần như không ai gọi từ tầng ứng dụng; cần thì `raw` |
| `LISTAGG` | cách viết của Oracle/DB2; `GROUP_CONCAT`/`STRING_AGG` đã phủ 3 dialect |
| `RATIO_TO_REPORT` | Oracle-only, không dialect nào hỗ trợ. **Đã gỡ variant khỏi enum** (P5) |
| `MERGE` | Oracle/SQL Server; đã bỏ từ trước |
| DDL (`CreateTable`, `DropTable`, `ForeignKeyBuilder`) | migration là việc của sqlx-migrate/refinery, không phải query builder |
| 12 dialect còn lại (Oracle, DB2, SQL Server, DB2, Firebird...) | mở lại chỉ khi có người dùng thật yêu cầu; kiến trúc template khiến chi phí thêm sau này vẫn thấp |
| `addFlag`/`addJoinFlag` theo vị trí cú pháp | escape hatch để bù tính năng thiếu của QueryDSL. Ở đây hoặc mô hình hoá hẳn tính năng, hoặc dùng `raw`. Có cả hai thì thừa |
| `Projections.bean/map/array`, `QBean`, `ConstructorExpression` | `#[derive(Projection)]` là dạng Rust-native của cùng nhu cầu, thay thế toàn bộ nhóm này |

Ghi chú: bỏ khỏi roadmap nghĩa là không lên kế hoạch, không phải cấm vĩnh viễn.

## Phases

| # | Nội dung | Vì sao ở vị trí này | Công sức |
|---|---|---|---|
| P1 | `#[derive(Projection)]` + GroupBy transformer | khoảng trống lớn nhất; là thứ biến thư viện từ "sinh chuỗi SQL" thành "dùng được cho ứng dụng" | **XONG** |
| P2 | Backend MySQL cho `oxider-query-exec` | xoá bất đối xứng builder-3 / exec-2 mà README đang phải cảnh báo ngay đầu file | **XONG** |
| P3 | `fetch_count` + kiểu kết quả phân trang | phân trang là nhu cầu phổ thông nhất chưa được phục vụ | **XONG** |
| P4 | Kiểu giá trị: decimal, uuid, json, array | không có bốn kiểu này thì nhiều schema Postgres thật không dùng được | **XONG** trừ array |
| P5 | `WITHIN GROUP` + gỡ `RatioToReport` | trả nợ API: 3 variant công khai mà mọi dialect đều từ chối | **XONG** |
| P6 | Thông báo lỗi biên dịch có hướng dẫn | khác biệt chỉ Rust mới làm được; rẻ và tác động trực tiếp tới trải nghiệm | Nhỏ |
| P7 | Codegen Postgres (+ PK/FK) | giá trị cao, công sức cũng cao nhất trong danh sách | Lớn |
| P8 | CI workflow | `make check` đã có, chỉ còn nối vào GitHub Actions | Rất nhỏ |
| P9 | Streaming + batch DML | chỉ cần khi có người dùng chạm trần hiệu năng | Vừa |

Hoãn, chưa xếp phase: **tài liệu tiếng Anh**. Đã chốt dịch máy toàn bộ, nhưng chưa làm bây giờ. Chi tiết và guard bắt buộc giữ ở mục riêng bên dưới để khi mở lại không phải nghĩ lại.

Phụ thuộc: P3 cần P2 (đã xong) để test trên cả ba backend. P8 dùng lại module `identifier` của codegen SQLite. Còn lại độc lập.

## P2. Backend MySQL - XONG

`crates/oxider-query-exec/src/mysql.rs`, một impl `Backend for sqlx::MySql`. `Db` và `Tx` không phải sửa một dòng nào, đúng như trait hứa.

Điểm phải quyết trong lúc làm: MySQL nhận date dưới dạng text như SQLite, nhưng không có literal datetime nào mang offset, và không có kiểu cột nào giữ được múi giờ.
Nên backend parse temporal ngược về `chrono` trước khi bind giống Postgres, còn `DateTime<Utc>` bị bỏ offset và bind theo **đồng hồ UTC** của nó.
Hệ quả ghi rõ trong module doc và `docs/12`: dùng `DATETIME` cho mốc thời gian, hoặc tự `SET time_zone = '+00:00'`.
Crate không tự đặt `time_zone` vì pool do sqlx dựng và `Db` không chen vào lúc mở kết nối - muốn làm thì phải thêm hook `connect` vào trait `Backend`, tức là đụng cả ba backend. Để lại thành việc riêng nếu có nhu cầu thật.

10 test E2E, `make mysql-up test-mysql mysql-down`.

Bắt được một bug có sẵn: macro `db_or_skip!` bind guard của mutex **bên trong nhánh `match`**, nên guard drop ngay tại đó và bộ test chưa bao giờ thực sự tuần tự hoá - đúng thứ commit `9c5ecf3` định sửa.
Trên MySQL nó hiện ra thành `Table 'oxider.ox_users' doesn't exist` ở 2 test. Đã sửa thành macro dạng statement, bind guard trong scope của test body, áp cho cả suite Postgres.

## P1. Projection và GroupBy transformer - XONG

Hai quyết định đã ghi ở dưới bị đảo lại trong lúc làm, vì có bằng chứng mới. Ghi lại cả hai để lần sau không bàn lại:

**1. `Projection` nằm ở `oxider-query-exec`, không phải core.**
Đọc hàng theo vị trí cần một trait mang `from_row_at(row, offset)`, mà `row` là `sqlx::Row`.
Core không phụ thuộc sqlx và không nên phụ thuộc, nên core chưa bao giờ là chỗ đặt được.
Tách `ARITY` sang core còn `from_row_at` ở exec thì thành hai nửa của một trait ở hai crate, tốn hơn được.

**2. `group_children` đi theo, cũng ở exec.**
Nó không cần gì từ exec (thuần std), nhưng nó là phép gấp trên chính các `Projection` đó. Một tính năng, một crate.
Lý do cũ - "facade chỉ có 10 dòng re-export, đừng bỏ logic vào" - vẫn đúng, chỉ là câu trả lời đổi từ core sang exec.

**3. Không kiểm tra được select ↔ projection lúc biên dịch.**
`Select` xoá projection thành `Vec<Node>` ngay khi dựng, nên không còn kiểu để đối chiếu.
Muốn có thì phải luồn tham số kiểu thứ tư qua mọi method của `Select`, đắt hơn giá trị nó mang lại.
Lệch nhau hiện ra thành lỗi chỉ số cột từ hàng đầu tiên; có test ghim đúng hành vi đó.

Cái ĐƯỢC kiểm tra lúc biên dịch: bề rộng của span cộng dồn qua tuple và `Option`, nên `(User, Option<Order>)` biết cắt hàng phẳng ở đâu mà không đọc hàng.

File: `exec/src/projection.rs`, `exec/src/transform.rs`, `macros/src/projection.rs`.
`oxider-query-macros` tách thành `lib.rs` / `attributes.rs` / `entity.rs` / `projection.rs` vì đã vượt 200 dòng.
`Db` và `Tx` thêm `fetch_all_projected` / `fetch_one_projected` / `fetch_optional_projected`; `fetch_all` cũ giữ nguyên đường `FromRow` theo tên.
Không thể làm blanket `impl FromRow for P: Projection` - trait ngoại + kiểu generic, vướng orphan rule - nên phải là method riêng chứ không phải cùng một `fetch_all`.

8 test E2E trên SQLite + 4 unit test cho phép gấp.

## Ghi chú thiết kế P1 (giữ nguyên bản gốc)

Hiện chỉ có tuple projection arity 2..=12.

### Làm rõ trước: `SelectionIn` và `Projection` là hai phía khác nhau

Kế hoạch cũ xếp cả hai vào một mục "projection và mapping", gây hiểu nhầm rằng chúng chồng nhau. Không phải.

- `SelectionIn` (`typed/selection.rs:58`) là **phía select-list**: kiểm tra phạm vi từng phần tử rồi đẩy ra `Vec<Node>`. Nó không đọc kết quả.
- `#[derive(Projection)]` là **phía đọc hàng**: dựng struct từ một hàng trả về.

Nên không có chuyện gộp. `SelectionIn` giữ nguyên, kể cả arity 2..=12.

### `#[derive(Projection)]`

**Khớp cột theo thứ tự trong `select`, không theo tên field.** Đã chốt.

Hệ quả kỹ thuật: **không dùng được `sqlx::FromRow` mặc định**, vì nó khớp theo tên cột.
Derive phải sinh giải mã theo chỉ số (`row.try_get(0)`, `try_get(1)`, ...), tức là tự sinh impl `FromRow` chứ không nhờ `#[derive(sqlx::FromRow)]`.

Kéo theo: `oxider-query-macros` phải phát ra đường dẫn `::sqlx::...`, nên phần đó nằm sau feature gate. Ai chỉ dựng SQL rồi tự bind không phải kéo sqlx vào.

Trait `Projection` (mang `ARITY` và danh sách kiểu ở tầng type, để đối chiếu với `select`) đặt ở `oxider-query-core`: `oxider-query-exec` chỉ phụ thuộc core, không phụ thuộc facade, nên mọi thứ exec phải gọi tên đều buộc phải ở core.

Lý do: thứ tự kiểm tra được ở tầng type (số field phải bằng arity của `select`, kiểu từng field phải khớp kiểu từng cột), còn khớp theo tên thì buộc mọi cột phải có alias và sai tên chỉ lộ ra lúc chạy.
Đổi thứ tự `select` mà quên đổi struct sẽ thành lỗi biên dịch nếu kiểu khác nhau, và thành `compile_fail` doc test cho trường hợp đó.

Hệ quả quan trọng: một tuple projection tiêu thụ cột theo **span liên tiếp**, nên `(User, Order)` đọc được từ một `select` phẳng - `User` lấy n cột đầu, `Order` lấy m cột tiếp theo.
Đây chính là thứ GroupBy transformer cần.

`Option<P>` là `None` khi **mọi** cột trong span của `P` đều NULL. Đó là quy ước cho `LEFT JOIN` không khớp.

### GroupBy transformer

Phương án đề xuất: **một hàm thuần tuý trong `oxider-query-core`, module `transform.rs`, không đụng tới `oxider-query-exec`.**

Chỗ đặt đã chốt là core, dù đúng là core khai trong charter chỉ có "AST, typed layer, builder, Dialect, renderer".
Lý do: `Projection` buộc phải ở core (xem trên), và `group_children` là phép gấp trên chính các `Projection` đó - tách một tính năng ra hai crate chỉ để giữ charter sạch thì trả giá nhiều hơn được.
Facade `oxider-query` không phải chỗ đặt: hiện nó là 229 dòng doc và đúng 10 dòng re-export, không có logic nào; đưa hàm này vào sẽ biến nó thành crate duy nhất vừa re-export vừa mang mã.
Charter của core sửa lại cho đúng thực tế thay vì bẻ thiết kế theo một câu mô tả.

```rust
pub fn group_children<P, C, K>(
    rows: impl IntoIterator<Item = (P, Option<C>)>,
    key: impl Fn(&P) -> K,
) -> Vec<(P, Vec<C>)>
where
    K: Eq + core::hash::Hash,
```

Dùng:

```rust
let rows: Vec<(User, Option<Order>)> = db.fetch_all(q).await?;
let tree: Vec<(User, Vec<Order>)> = group_children(rows, |u| u.id);
```

Bốn quyết định trong chữ ký đó:

1. **Nhận kết quả đã fetch, không tích hợp vào `Db::fetch_all`.** Giữ lõi độc lập execution - đây là bất biến đã có của workspace, không đánh đổi lấy một dòng tiện hơn. Ai render rồi tự bind bằng driver của mình, không qua `Db`, vẫn dùng được transformer.
2. **Trả `Vec<(P, Vec<C>)>` chứ không phải `HashMap`.** `ORDER BY` phải sống sót qua phép gấp. Bên trong dùng `HashMap<K, usize>` trỏ vào chỉ số của `Vec`, nên vẫn O(n) mà không kéo thêm dependency `indexmap`.
3. **Key là closure, không phải biểu thức cột.** QueryDSL phải nhận `Expression` vì Java không có closure rẻ. Rust có, và closure thì kiểm tra kiểu chặt hơn.
4. **`Option<C>` nằm trong chữ ký, `None` bị bỏ qua.** `LEFT JOIN` không khớp phải cho `Vec` rỗng chứ không phải một phần tử rác.

Phạm vi: **hai tầng**. Lồng ba tầng (`User` → `Order` → `OrderLine`) hoãn tới khi có case thật; ghép hai lần vẫn ra kết quả, chỉ là chưa gọn.

Đây là tính năng được dùng nhiều thứ hai của QueryDSL sau bản thân builder, và là lý do người ta dùng nó thay vì viết SQL tay.

## P3. Đếm và phân trang - XONG

`Select::count()` ở core **bọc** query (`SELECT COUNT(*) FROM (...) AS "oxider_count"`) chứ không thay projection.
Bọc mới đúng với `DISTINCT`/`GROUP BY`/set op - có E2E chứng minh: 3 hàng, 2 tuổi khác nhau, `DISTINCT` đếm ra 2.
Bỏ `LIMIT`/`OFFSET` và `ORDER BY`, trừ khi có `DISTINCT ON` vì Postgres bắt hai thứ đó phải khớp nhau.

`Page<T>` ở exec: `items`, `total`, `size`, `number`, cộng `total_pages()`/`has_next()`/`has_previous()`.
`fetch_page` đếm trước rồi fetch, hai lượt đi về.
Cố ý không dùng `COUNT(*) OVER ()`: một lượt nhưng trang vượt cuối thì không có hàng nào để gắn số đếm, mất luôn tổng - đúng lúc cần nó nhất. Có test ghim.

`fetch_count`/`fetch_page`/`fetch_page_projected` trên cả `Db` lẫn `Tx`.

## P4. Kiểu giá trị - XONG (trừ array)

Làm đúng thiết kế ghi ở mục dưới, không đổi gì: `Value::Decimal/Uuid/Json` giữ text chuẩn hoá, enum không đổi hình theo feature, `ToSqlValue` nằm sau feature `rust_decimal`/`uuid`/`json`.
Markers: Decimal là `Numeric`, Uuid là `Orderable`, Json chỉ `SqlType`.

Quy tắc bind chốt ở `exec/src/scalars.rs`: **kiểu native ở engine nào có, text ở engine nào không.**
Ngoại lệ duy nhất trông có vẻ thiếu nhất quán là UUID trên MySQL, bind bằng text chứ không bằng `Uuid` của sqlx: kiểu đó mã hoá `BINARY(16)`, đem ghi vào cột `CHAR(36)` là ghi byte rác mà không báo lỗi.

Phát hiện trong lúc chạy test thật, đã ghim thành test chứ không giấu: **SQLite không có decimal chính xác.**
Cột `NUMERIC` sắp xếp đúng nghĩa số học nhưng làm tròn qua `REAL`; cột `TEXT` giữ đủ chữ số nhưng so sánh theo chuỗi (`"10.25" < "9.5"`).
Không có lựa chọn thứ ba, và cả hai nửa đều có test riêng để không ai đọc nửa này mà tưởng là khuyến nghị.

Feature `chrono` cố ý **không** có passthrough ở facade.
Dependency kế thừa từ bảng workspace đã bật default của core, nên `default-features = false` viết ở facade không có tác dụng; một feature không tắt được thứ nó nói thì không nên tồn tại.

Nợ phát sinh đã trả luôn: `.PHONY` của Makefile từ P2 chứa một ký tự `\n` viết nhầm thành literal, và ba target `mysql-up`/`mysql-down`/`test-mysql` được khai báo ở đó nhưng chưa bao giờ được viết.
Nay có đủ, cộng `test-db` chạy cả ba suite; hai target `test-pg`/`test-mysql` đổi sang `--all-features` vì với `--features postgres` thì test value kind bị cfg loại khỏi bản dịch mà suite vẫn báo xanh.

Array Postgres tách ra làm sau: nó cần thêm toán tử (`= ANY`, `@>`, `&&`) chứ không chỉ thêm kiểu.

## Ghi chú thiết kế P4 (giữ nguyên bản viết trước khi làm)

Phát hiện quan trọng: `Value` **không phụ thuộc feature flag**.
`Date`/`Time`/`DateTime` giữ `String` text chuẩn hoá (`value.rs:33-38`); feature `chrono` chỉ thêm impl `ToSqlValue` format vào đó; backend Postgres parse ngược lại trước khi bind (`postgres.rs:66-77`).

Nên câu hỏi "rust_decimal hay bigdecimal" là câu hỏi sai. Làm theo đúng khuôn đó:

- Thêm `Value::Decimal(String)`, `Value::Uuid(String)`, `Value::Json(String)` - giữ text chuẩn hoá, lõi vẫn không phụ thuộc thư viện nào.
- Feature `rust_decimal` thêm `impl ToSqlValue for Decimal`; feature `bigdecimal` thêm impl cho `BigDecimal`. **Bật cả hai cùng lúc không xung đột**, vì chỉ là hai impl khác nhau đổ vào cùng một variant. Enum không đổi hình dạng theo feature.
- Backend Postgres parse text ngược thành kiểu đã bật trước khi bind, y hệt cách `Stamp` đang xử lý timestamp.
- `impl Numeric` cho decimal thì toàn bộ toán tử số học có sẵn, không phải khai báo lại.

**Đề xuất mặc định: `rust_decimal`.**
`Copy`, không cấp phát, phủ mọi `NUMERIC(p,s)` với p ≤ 28 (đủ cho tiền tệ và gần như mọi cột thật), và là lựa chọn phổ biến nhất trong hệ sinh thái sqlx.
`bigdecimal` để sau feature riêng cho ai cần `NUMERIC` không giới hạn của Postgres.
Không đặt cái nào vào feature mặc định: `chrono` đang bật sẵn, thêm nữa là kéo dependency cho người không dùng.

Array Postgres cần thêm toán tử (`= ANY`, `@>`, `&&`) chứ không chỉ kiểu, nên tách thành hạng mục con làm sau decimal/uuid/json.

## P5. `WITHIN GROUP` - XONG

Ba variant hứa suông nay còn hai variant chạy được và một variant bị gỡ.
`RatioToReport` xoá khỏi enum: Oracle-only, không dialect nào có.
`PercentileCont`/`PercentileDisc` chuyển từ `Family::Window` sang `Family::Aggregate` - chúng là ordered-set aggregate, nên hợp lệ trong `HAVING` và nhận được `FILTER`, đúng như bản chất.

Không thêm cờ nào lên node. Hình dạng `WITHIN GROUP` suy ra từ chính operator, nên có `Operator::is_ordered_set()` và renderer đọc từ đó; một cờ trên node là một chỗ nữa để hai nguồn sự thật nói khác nhau.

API: `percentile_cont(f)` trả về builder chứ **không** trả về `Aggregate`.
Lý do là `WITHIN GROUP` bắt buộc trong mọi engine, nên bản viết dở phải không biên dịch được, thay vì render ra SQL không parse.
`within_group` nhận thẳng biểu thức chứ không nhận `Order<S>`, vì `Order<S>` xoá mất `T` mà `percentile_disc` cần giữ để trả về đúng kiểu của cột.
Chiều giảm dần thành method thứ hai `within_group_desc`; với percentile rời rạc nó không phải lúc nào cũng bằng `1 - f` tăng dần nên không thể bỏ.

`Caps::ordered_set_aggregates` là mục đầu tiên trong Caps **không có đường giả lập nào**, và ghi rõ lý do ngay tại chỗ khai báo: trung vị là tính chất của cả nhóm đã sắp, không biểu thức trên một hàng nào dựng lại được.

Đúng như dự đoán khi lập kế hoạch: phạm vi thật chỉ PostgreSQL. Test E2E trên Postgres thật chứng minh phần mà render không chứng minh được - trên các tuổi 10/20/30/40 thì `cont` cho 25.0 (không hàng nào có) còn `disc` cho 20 (có thật trong nhóm).

## Ghi chú thiết kế P5 (giữ nguyên bản viết trước khi làm)

`Aggregate` đã mang sẵn `order_by` (`typed/aggregate.rs:24`), và render tập trung ở một hàm `Renderer::aggregate` (`render/expr.rs:60`).
Việc cần làm: một cờ trên aggregate cho biết `order_by` render thành `WITHIN GROUP (ORDER BY ...)` ngoài ngoặc thay vì trong ngoặc, một `Caps` mới, và template cho `PERCENTILE_CONT`/`PERCENTILE_DISC`.

Phạm vi thật: **chỉ Postgres**. MySQL 8 và SQLite đều không có ordered-set aggregate, nên cả hai từ chối.
Biết trước điều này thì đừng kỳ vọng nhiều: giá trị chính của P5 là xoá 3 variant hứa suông, không phải tính năng mới.

`RatioToReport` gỡ hẳn, không dialect nào có.

## P7. Thông báo lỗi biên dịch

Kiểm tra phạm vi ở tầng type là điểm mạnh nhất của thư viện, nhưng hiện nó nói:

```text
error[E0277]: the trait bound `Nil: Contains<Department, _>` is not satisfied
```

Với `#[diagnostic::on_unimplemented]` trên `Contains`, `ContainsAll`, `IntoExpr` và `Numeric`, câu đó thành lời khuyên đọc được: bảng nào thiếu, thêm join nào.
Rẻ, không đổi kiến trúc, và là loại đánh bóng chỉ Rust mới có - đúng tinh thần mục tiêu mới.

## Hoãn: tài liệu tiếng Anh

16 chương hiện chỉ có tiếng Việt.
Với mục tiêu "tốt nhất cho Rust", người đọc ở docs.rs và crates.io, nên đây là rào cản tiếp nhận lớn hơn mọi tính năng còn thiếu trong danh sách này.

Dùng i18n của Docusaurus, tiếng Anh làm ngôn ngữ mặc định của site, tiếng Việt là bản dịch.
Rustdoc và README đã là tiếng Anh nên không phải đụng.

**Dịch máy toàn bộ 16 chương.** Đã chốt. Không soát tay từng câu.

Rủi ro thật của cách này không phải văn phong mà là **dịch máy sửa vào code**: đổi tên biến trong khối ```rust, dịch từ khoá trong khối ```sql, đổi `LIKE` thành một từ tiếng Anh khác, hay bẻ dấu nháy trong chuỗi.
Hiện không có gì bắt được: `documented_example_scenarios.rs` assert SQL trong mã Rust của test, không đọc file docs, nên một khối code hỏng trong docs vẫn để test xanh.

Nên P6 kèm một guard rẻ: script trích mọi khối ```rust và ```sql từ `docs/` và từ bản dịch, rồi assert **giống nhau từng byte**.
Prose được phép khác, code thì không. Nối vào `make check`.

Guard này phải làm **trước** khi chạy dịch, không phải sau.

## Acceptance criteria

| Tiêu chí | Áp dụng cho |
|---|---|
| `make check` xanh | mọi phase |
| Mọi tính năng SQL mới có scenario test ghim đủ chuỗi SQL và params cho mọi dialect diễn đạt được | P4, P5 |
| Tính năng dialect không hỗ trợ trả `Err` và có test chứng minh | P5 |
| Case khó có E2E chạy DB thật | P2, P3, P4 |
| Mọi bảo đảm ở tầng type có doc test `compile_fail` | P1, P7 |
| Không variant `Operator` nào không render được ở ít nhất một dialect | P5 |
| Đoạn SQL in trong docs được ghim bằng test | P6 |

## Việc dọn dẹp kèm theo

1. Sửa bảng trạng thái trong `plans/260905-2058-querydsl-full-port/plan.md`: Phase 7 ghi "CHƯA" nhưng backend Postgres đã có từ commit `fa4b8ef`, chạy trong v0.2.0 trở lên.
2. Sửa mục Roadmap trong `README.md`: đang trỏ vào plan cũ và liệt kê việc còn lại theo mục tiêu cũ.

## Câu hỏi chưa giải quyết

1. `#[derive(Projection)]` tự sinh impl `FromRow` theo chỉ số thì mất `#[derive(sqlx::FromRow)]` mà người dùng đang quen. Có giữ đường thoát cho ai muốn khớp theo tên không, hay ép positional để giữ đúng một quy tắc?
2. Feature gate cho phần sinh mã sqlx đặt ở `oxider-query-macros` hay ở facade `oxider-query`? Macro crate không thấy được feature của crate gọi nó.
3. Khi mở lại việc dịch: dùng công cụ nào, commit bản dịch vào repo hay sinh lúc build?

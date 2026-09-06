# QueryDSL Templates + Serialization Architecture

Research target: how an operator AST becomes dialect-specific SQL text.
Scope: `querydsl-core` types/support layer plus `querydsl-sql` dialect layer.
All paths below are relative to `querydsl-libraries/` in the repo root.

## 1. The Template mechanism

### 1.1 Core files

- `querydsl-core/src/main/java/com/querydsl/core/types/Template.java`
- `querydsl-core/src/main/java/com/querydsl/core/types/TemplateFactory.java`
- `querydsl-core/src/main/java/com/querydsl/core/types/Templates.java`
- `querydsl-core/src/main/java/com/querydsl/core/types/TemplateExpression.java`
- `querydsl-core/src/main/java/com/querydsl/core/types/OperationImpl.java`

### 1.2 Template string syntax

A template is a plain Java string with `{...}` placeholders.
`TemplateFactory` parses it once (cached, `WeakHashMap<String, Template>`) into a list of `Template.Element`.
The parser is a single regex in `TemplateFactory.java`:

```
\{(%?%?)(\d+)(?:([+-/*])(?:(\d+)|'(-?\d+(?:\.\d+)?)'))?([slu%]?%?)\}
```

Groups and meaning:

| Group | Name | Meaning |
|---|---|---|
| 1 | premodifiers | `%` or `%%` before the index: "ends with via like" / "ends with via like, lower-cased" |
| 2 | index | argument index (0-based) into the operation's arg list |
| 3 | math op | one of `+ - / *`, only when followed by group 4 or 5 |
| 4 | index2 | second argument index for a binary math sub-expression, e.g. `{0+1}` |
| 5 | const number | numeric literal for math with a constant, e.g. `{1+'1'}` or `{2-'1'}` |
| 6 | postmodifiers | `s` (as string), `l` (lower), `u` (upper), `%` / `%%` (starts-with / starts-with-lower, or contains/contains-lower when combined with a premodifier) |

Concrete examples pulled from `Templates.java` and `SQLTemplates.java`:

- `"{0}, {1}"` - `Ops.LIST`, plain arg concatenation with static text.
- `"{0}.{1s}"` - `PathType.PROPERTY`: arg 1 rendered `asString` (a property name, not further SQL-escaped as an expression).
- `"{0} like {1} escape '{2s}'"` - `Ops.LIKE_ESCAPE`.
- `"{0l} like {1l} escape '{2s}'"` - `Ops.LIKE_ESCAPE_IC`: `l` postmodifier lower-cases both operands for case-insensitive LIKE.
- `"{0} like {%1} escape '\\'"` - `SQLTemplates` default `Ops.ENDS_WITH`: `%` before the index means "prefix the value with a wildcard `%`" (ends-with).
- `"{0} like {1%} escape '\\'"` - `Ops.STARTS_WITH`: `%` after the index means "suffix the value with `%`".
- `"{0l} ilike {%%1%%} escape '\\'"` (Postgres shape) - `%%` combos mean contains + lower-case.
- `"cast({0} as {1s})"` - `SQLOps.CAST`.
- `"substr({0},{1+'1's},{2-1s})"` - SQL default `Ops.SUBSTR_2ARGS`: constant-folds `arg1 + 1` and `arg2 - 1` (0-based to 1-based index conversion for SQL `SUBSTR`) and renders each as a string.
- `"strpos(repeat('^',{2-'1's}) || substr({1},{2s}),{0})"` (Postgres `LOCATE2`) - reuses arg index 2 twice in one template with two different modifiers (`{2-'1's}` and `{2s}`).

### 1.3 `Template.Element` subtypes (`Template.java`)

- `StaticText` - literal text, `isString() == true`.
- `ByIndex` - raw argument passthrough; if the arg is an `Expression`, it is unwrapped via `ExpressionUtils.extract`.
- `AsString` - like `ByIndex` but constants are converted with `.toString()` instead of staying typed (used for identifiers/type names/escape chars embedded as literal SQL text, not bind parameters).
- `Transformed` - wraps a `Function<Object,Object>` (the `toLowerCase`/`toUpperCase`/`toStartsWithViaLike*`/`toEndsWithViaLike*`/`toContainsViaLike*` transformers built in `TemplateFactory`). For a `Constant` argument it pre-escapes and folds the value into a new constant string (e.g. `"foo"` becomes `"foo%"` with `%`/`_`/escape-char escaped via `escapeForLike`); for an `Expression` argument it instead builds a `Ops.CONCAT` sub-`Operation` at serialization time (`expr + '%'`), because a non-constant value cannot be pre-escaped.
- `Operation` / `OperationConst` - the `{0+1}` / `{0+'1'}` math forms. If both operands are actually numeric (`Number` or numeric `Constant`) the result is **constant-folded immediately** via `MathUtils.result` (`querydsl-core/.../util/MathUtils.java`, backed by `BigDecimal` arithmetic) instead of emitting SQL arithmetic. If not, it builds a `com.querydsl.core.types.Operation` sub-tree (e.g. `path + 1`) that the serializer then recurses into normally. There is also an `CONVERTIBLES = {ADD, SUB}` special case that algebraically combines a chained `(x + a) - b` into a single `x OP (a-b)` if the constant math permits it.

### 1.4 Precedence and parenthesization

Precedence is **not** part of `Template`; it lives in a parallel map on `Templates`:

```java
// Templates.java
protected static class Precedence {
  public static final int HIGHEST = -1;
  public static final int DOT = 5;
  public static final int NOT_HIGH = 10;
  public static final int NEGATE = 20;
  public static final int ARITH_HIGH = 30;
  public static final int ARITH_LOW = 40;
  public static final int COMPARISON = 50;
  public static final int EQUALITY = 60;
  public static final int CASE = 70, LIST = 70;
  public static final int NOT = 80;
  public static final int AND = 90;
  public static final int XOR = 100, XNOR = 100;
  public static final int OR = 110;
}
private final Map<Operator, Template> templates = new IdentityHashMap<>(150);
private final Map<Operator, Integer> precedence = new IdentityHashMap<>(150);
```

Lower number = binds tighter (mirrors Java operator precedence, hence the name/values).
`add(op, pattern)` with no explicit level defaults precedence to `-1` (`HIGHEST`, meaning "not applicable" - used for function-call-shaped operators like `count({0})` or `cast(...)` where the parens are already provided by the syntax and no ambiguity can arise).
`add(op, pattern, precedenceInt)` sets an explicit level for operators that appear infix/without surrounding parens (`AND`, `OR`, `+`, `<`, ...).
Dialects also call `setPrecedence(int, Operator...)` to retune existing operators without touching their templates (see section 4).

Parenthesization is decided at serialization time in `SerializerBase.visitOperation` (`querydsl-core/src/main/java/com/querydsl/core/support/SerializerBase.java`), not in `Template`:

```java
if (precedence > -1 && expr instanceof Operation<?> operation) {
  var opPrecedence = templates.getPrecedence(operation.getOperator());
  if (precedence < opPrecedence) {
    append("(").handle(expr).append(")");
  } else if (!first && precedence == opPrecedence && !SAME_PRECEDENCE.contains(op)) {
    append("(").handle(expr).append(")");
  } else {
    handle(expr);
  }
}
```

Rules: if the *parent* operator's own precedence is `-1`, no wrapping logic runs at all for its children (function-call context).
Otherwise, a child sub-`Operation` is wrapped in parens if its precedence number is strictly greater (binds looser) than the parent's, or if it ties the parent's precedence and it is not the first operand and not in the `SAME_PRECEDENCE` set (`CASE`/`CASE_WHEN`/`CASE_ELSE`/`CASE_EQ*`, which share precedence 70 but must not be parenthesized against each other). This reproduces standard left-associativity without an explicit associativity flag.

`TemplateExpression` (`core/types/TemplateExpression.java`) is the interface any ad hoc/custom expression implements to carry its own `Template` + arg list (`Expressions.template(...)`/`Expressions.stringTemplate(...)` in `core/types/dsl/Expressions.java` build these directly against `TemplateFactory.DEFAULT`, independent of any `Templates` registry - so user code can drop raw SQL fragments outside the operator-precedence system entirely). `OperationImpl` (`core/types/OperationImpl.java`) is the default `Operation<T>` node: immutable `(type, operator, List<Expression<?>> args)`, dispatches via `Visitor.visit(Operation, C)`.

Note: `Templates` itself is reused outside SQL - `core/types/JavaTemplates.java` extends it to render the same AST as a Java boolean expression (used by the in-memory `querydsl-collections` evaluator), confirming the template/precedence system is deliberately backend-agnostic; only `SQLTemplates` (section 3) adds SQL-specific vocabulary and knobs.

## 2. SerializerBase: AST walk, SQL emission, bind parameters

File: `querydsl-core/src/main/java/com/querydsl/core/support/SerializerBase.java` (implements `Visitor<Void, Void>`).

- `visit(Operation, ctx)` -> `visitOperation(type, operator, args)`: looks up `Template` + precedence for the operator, then for each `Template.Element` calls `element.convert(args)` and dispatches on the result: `Expression` -> recurse (`handle(expr)`, with the paren logic above); `element.isString()` -> `append(rv.toString())` straight into the internal `StringBuilder`; otherwise -> `visitConstant(rv)` (a folded/derived constant, e.g. from `OperationConst`).
- `visit(Path, ctx)`: looks up the `Template` for the path's `PathType` (`PROPERTY`, `VARIABLE`, `ARRAYVALUE`, `COLLECTION_ANY`, ...) and runs it through the same `handleTemplate` machinery - paths are serialized with templates too, not a special case.
- `visit(TemplateExpression, ctx)`: directly `handleTemplate(expr.getTemplate(), expr.getArgs())` - this is the path ad hoc `Expressions.template(...)` calls take.
- `visit(Constant, ctx)` -> `visitConstant(constant)`.
- `visit(ParamExpression, ctx)`: named/anonymous bind parameters get a label (`paramPrefix + name` or `anonParamPrefix + name`) registered directly into `constantToLabel`/`constants` and rendered via `serializeConstant`.
- `visit(FactoryExpression, ctx)`: comma-joins the constituent args (used for tuple/bean projections).

Constant handling (base class): `visitConstant(Object)` computes a label via `getConstantToLabel().computeIfAbsent(constant, this::getConstantLabel)` (`getConstantLabel` default = `constantPrefix + ordinal`, e.g. `a1`, `a2`), appends the constant object to the `constants` list (exposed via `getConstants()`), then calls the overridable `serializeConstant(index, label)` (base default just appends the label text - meaningful for engines that inline named/labelled constants; SQL overrides this, see below).

SQL-specific override, `querydsl-sql/src/main/java/com/querydsl/sql/SQLSerializer.java`:

- `serializeConstant(...)` is overridden to always `append("?")` - **every** dialect uses plain JDBC positional placeholders; there is no dialect-specific placeholder syntax (`$1`, `:name`, `@p1`) anywhere in this layer, because binding happens through `PreparedStatement` regardless of DB.
- `visitConstant(Object)` is overridden to add SQL-specific normalization:
  - `useLiterals == true` (opt-in via `Configuration.setUseLiterals`/`SQLSerializer.setUseLiterals`, used for logging/debug SQL, not execution): renders the value directly as SQL literal text via `Configuration.asLiteral(o)`, which looks up the registered `Type<T>` (`sql/types/*`) for the Java class, gets its raw literal string, then calls `SQLTemplates.serialize(literal, jdbcType)` for dialect-specific literal quoting/casting (see section 3/4). `null`/`Null` -> `"null"`.
  - `Collection` constants (IN-lists): parenthesized, comma-joined; each element gets its own `?`/constant slot (or literal, if `useLiterals`), and `constantPaths` tracks which `Path` each bound value belongs to (for JDBC type inference on `setObject`).
  - Plain scalar in `SELECT` position with `templates.isWrapSelectParameters() == true` (Firebird only, see section 3): wraps the bare `?` in `CAST(? AS <typename>)` because that driver cannot infer an untyped positional parameter's type when it's a bare projected column.
  - Otherwise: normal path, register constant + `?`.

"%"-escaping for LIKE, concretely: this happens in two layers.

1. At template-parse time (`TemplateFactory`, section 1.3): the `Transformed` functions (`toStartsWithViaLike`, `toEndsWithViaLikeLower`, `toContainsViaLike`, etc.) call `escapeForLike(str)`, which prefixes every occurrence of the escape char, `%`, or `_` in a **constant** operand with the configured escape character, then wraps the result with the literal `%` the LIKE-family operator needs. `TemplateFactory.escapeForLike` uses `\\` by default; `Templates.escapeForLike` (overridden and delegated to by the per-`Templates`-instance factory) is the version actually used at runtime, parameterized by the dialect's `escape` char (see `getEscapeChar()` / constructor arg).
2. At operation-visit time (`SQLSerializer.visitOperation`, `Ops.LIKE` branch): if the RHS of a raw `LIKE` is itself a `Constant` (user passed a literal pattern, not `.startsWith()`/`.contains()`), the escape character occurring in that literal text is doubled (`escape` -> `escape+escape`) before re-visiting as `Ops.LIKE`, since the driver still needs the escape char itself escaped once more when it appears literally in the pattern.

Known gap surfaced by this design: when the LIKE-family operand is a **dynamic expression** (not a compile-time constant), `Transformed.convert` cannot pre-escape it (it doesn't have the runtime string yet), so it just does `CONCAT(expr, '%')` with no escaping of `%`/`_` inside the expression's runtime value. This is a real, source-verified behavior difference between constant and expression patterns worth deciding on for the Rust port (fix, document, or intentionally mirror).

## 3. SQLTemplates: behavioral knobs

Files: `querydsl-sql/src/main/java/com/querydsl/sql/SQLTemplates.java` (1273 lines), `SQLSerializer.java` (1210 lines), `SQLTemplatesRegistry.java` (94 lines), `Configuration.java` (604 lines).

`SQLTemplatesRegistry.getBuilder(DatabaseMetaData)` (`SQLTemplatesRegistry.java`) is the JDBC-metadata-driven dialect picker: matches `getDatabaseProductName()` (lowercased) against a fixed string table (`postgresql`, `mysql`, `oracle`, `sqlite`, `h2`, `hsql`, `apache derby`, `cubrid`, `firebird*`, `turso`, `teradata*`) and falls back to a generic ANSI `SQLTemplates` for anything else; SQL Server is special-cased further by `getDatabaseMajorVersion()` into `SQLServerTemplates` (<9), `SQLServer2005Templates` (9), `SQLServer2008Templates` (10), or `SQLServer2012Templates` (>=11).

`Configuration` (`Configuration.java`) is the outer object apps construct: holds one `SQLTemplates` instance, the `JDBCTypeMapping`/`JavaTypeMapping` (Java<->JDBC type registries used by `getTypeNameForCast`/`asLiteral`), table/column/schema name overrides, `SQLListeners`, and the `useLiterals` toggle. `SQLSerializer` is constructed per-query from a `Configuration` and delegates almost everything dialect-specific to `conf.getTemplates()`.

### 3.1 Knob catalog

"Default" = value set in the base `SQLTemplates` constructor unless noted. Booleans/strings only; template-map entries (`add(Ops.X, "...")`) are catalogued separately in section 4 per operator family.

| Knob (getter) | Default | Dialects that override -> value |
|---|---|---|
| `quoteStr` (ctor arg, used by `quoteIdentifier`) | `"` | MySQL: `` ` ``. All others read: `"`. |
| escape char (`getEscapeChar`, ctor arg) | `\\` | All sampled dialects keep `\\`; templates conditionally simplify LIKE patterns when `escape == '\\'` (Postgres, MySQL). |
| `limitTemplate` | `"\nlimit {0}"` | DB2, Derby: `"\nfetch first {0s} rows only"`. CUBRID keeps field but overrides `serializeModifiers` entirely (own `offsetLimitTemplate`). Oracle, SQLServer*, DB2, CUBRID, Teradata override `serializeModifiers` and/or `serialize(...)` to bypass this field completely (custom ROWNUM/TOP/window-function pagination, see section 4). |
| `offsetTemplate` | `"\noffset {0}"` | DB2: `"rn > {0}"` style (subquery). Derby: `"\noffset {0s} rows"`. Firebird/Oracle/SQLServer2005/2012/Teradata/CUBRID: own strings or method overrides entirely. |
| `limitRequired` | `false` | MySQL, SQLite, HSQLDB, H2: `true` (LIMIT is syntactically mandatory before OFFSET, or used as a safety cap). |
| `dummyTable` (`SELECT ... FROM <dummy>` for no-FROM selects) | `"dual"` | Postgres, SQLite, Teradata, CUBRID: `null` (no dummy table needed). DB2, Derby: `"sysibm.sysdummy1"`. Firebird: `"RDB$DATABASE"`. SQLServer: `""`. |
| `nullsFirst` / `nullsLast` | `" nulls first"` / `" nulls last"` | MySQL, SQLite, SQLServer (+2005/2008/2012), CUBRID, Teradata, DB2: both `null` (native syntax unsupported; `SQLSerializer.handleOrderBy` falls back to `CASE WHEN x IS NULL THEN 0/1 ELSE 1/0 END` emulation). Postgres, Oracle, HSQLDB, Derby, Firebird, H2: keep native support. |
| `forShareSupported` | `false` | Postgres: `true` (`FOR SHARE`). MySQL: `true` (`LOCK IN SHARE MODE`, custom `forShareFlag`). SQLServer: `true` (`WITH (REPEATABLEREAD)` table hint, custom flag position `BEFORE_FILTERS`). |
| `forUpdateFlag` / `forShareFlag` / `noWaitFlag` (position + text) | `Position.END`, `"\nfor update"` / `"\nfor share"` / `" nowait"` | SQLServer: repositions both to `BEFORE_FILTERS` and swaps text to `WITH (UPDLOCK)` / `WITH (REPEATABLEREAD)` table hints instead of trailing clauses. |
| `nativeMerge` | `false` | H2: `true` (emits native `MERGE INTO ... KEY(...) VALUES (...)`; all others emulate MERGE via ANSI `MERGE ... USING (...) ON (...) WHEN MATCHED/NOT MATCHED`). |
| `batchToBulkSupported` | `true` | SQLite, Firebird, Teradata: `false` (no bulk multi-row rewrite; issued as separate statements). |
| `arraysSupported` | `true` | MySQL, SQLServer, SQLite, CUBRID: `false` (no native SQL `ARRAY` type). |
| `countDistinctMultipleColumns` | `false` | Postgres, H2: `true` (`COUNT(DISTINCT a, b)` is valid syntax). |
| `countViaAnalytics` | `false` | Postgres, Oracle, Teradata: `true` (paginated queries fetch total count via a window-function `count(*) over()` column instead of a second `COUNT(*)` query). |
| `wrapSelectParameters` | `false` | Firebird: `true` (bare `?` in a projected column must be `CAST(? AS <type>)`; driver can't infer type otherwise). |
| `functionJoinsWrapped` | `false` | DB2, HSQLDB: `true` (table-valued function calls in `FROM`/`JOIN` wrapped as `TABLE(...)`). |
| `unionsWrapped` | `true` | SQLite, Firebird, HSQLDB: `false` (UNION branches emitted without wrapping parens). |
| `supportsUnquotedReservedWordsAsIdentifier` | `false` | Postgres, MySQL: `true` (relaxes quoting requirement for a reserved word immediately after a `.`). |
| `listMaxSize` (max IN-list literal count before auto-partitioning into OR'd chunks) | `0` (unlimited) | Oracle: `1000` (Oracle's historical 1000-element IN-list limit). |
| `maxLimit` (used with `limitRequired` when no explicit `.limit()` is set) | `Integer.MAX_VALUE` | H2: `2 ^ 31` - this is Java's XOR operator, not exponentiation, so it actually evaluates to `29`. Verified in source (`H2Templates.java`); flagged here as a real upstream oddity, not something to imitate. |
| `parameterMetadataAvailable` | `true` | MySQL, Oracle: `false` (driver's `ParameterMetaData` unreliable; affects how null bind values get typed elsewhere in `querydsl-sql`). |
| `batchCountViaGetUpdateCount` | `false` | Oracle: `true`. |
| `requiresSchemaInWhere` | `false` (ctor param) | Derby: `true`. |
| `getCastTypeNameForCode(int)` (JDBC type code -> dialect cast-target name) | delegates to `getTypeNameForCode` | MySQL: `signed`/`decimal`/`char`. Oracle: `varchar(4000 char)`/`double precision`. DB2, Teradata: `varchar(4000)`. HSQLDB: `varchar(10)`. Firebird: `varchar(256)`. |
| `serialize(String literal, int jdbcType)` (literal formatting for `useLiterals`/DDL) | quotes strings, passes numerics through, wraps `timestamp '...'`/`date '...'`/`time '...'` | Postgres: `1`/`0` -> `true`/`false` for `BOOLEAN`. SQLServer: `CAST('...' AS DATETIME2/DATETIMEOFFSET/DATE/TIME)`. Oracle: `timestamp '...'`, `date '...'`, and a synthetic `1970-01-01` date prefix for bare `TIME`. SQLite: converts ISO literal to epoch-millis (its `TIMESTAMP`/`DATE`/`TIME` types are stored as integers). Derby: JDBC-escape braces `{ts '...'}`/`{d '...'}`/`{t '...'}`, plus `1`/`0` -> `true`/`false`. CUBRID: `timestamp'...'` (no space) / `date'...'` / `time'...'`. HSQLDB: strips the space HSQLDB doesn't tolerate before a timezone offset. |
| `isSupportsAlias()` | `true`, hardcoded `final` | Never overridden - column/table aliasing is assumed universal. |
| RETURNING clause | **not modeled at all** | Confirmed by full-text search: no `RETURNING`/`returning` string anywhere in `querydsl-sql/src/main/java`. Generated keys are retrieved purely through JDBC (`Statement.RETURN_GENERATED_KEYS` + `getGeneratedKeys()`, see `dml/AbstractSQLInsertClause.java`), not via a dialect SQL clause. This is a deliberate simplification in this codebase, not an oversight to reproduce blindly - see section 5. |
| Upsert convenience | Not a `Templates` knob | MySQL only, and only as a query-factory helper: `mysql/MySQLQueryFactory.insertOnDuplicateKeyUpdate(...)` appends a raw ` on duplicate key update ...` string as a trailing `QueryFlag`. Postgres `ON CONFLICT` has no equivalent helper anywhere in the sampled source. |

## 4. Per-dialect differences: Postgres / MySQL / SQLite / SQL Server / Oracle

Sources: `PostgreSQLTemplates.java`, `MySQLTemplates.java`, `SQLiteTemplates.java`, `SQLServerTemplates.java` + `SQLServer2005/2008/2012Templates.java`, `OracleTemplates.java`.

| Aspect | Postgres | MySQL | SQLite | SQL Server (default/pre-2005; 2012 in parens) | Oracle |
|---|---|---|---|---|---|
| Identifier quote char | `"` | `` ` `` | `"` (base default) | `"` (this codebase; real SQL Server also accepts `[...]`, not modeled here) | `"` |
| Bind placeholder | `?` (JDBC) | `?` | `?` | `?` | `?` |
| LIMIT/OFFSET | `LIMIT {0}` / `OFFSET {1}` (ANSI-ish, base templates, `limitRequired=false`) | `LIMIT {0}` / `OFFSET {1}`, `limitRequired=true` | `LIMIT {0}` / `OFFSET {1}`, `limitRequired=true` (SQLite needs `LIMIT` present for `OFFSET` to be valid) | Pre-2012: `TOP {0}` for limit-only (`topTemplate`), `ROW_NUMBER() OVER(...)` subquery rewrite (`rn > offset`) for limit+offset (2005+), throws `IllegalStateException` on offset-without-2005-rewrite in the base `SQLServerTemplates`. 2012: native `OFFSET {1} ROWS FETCH NEXT {0} ROWS ONLY`, but only if an `ORDER BY` exists; falls back to `TOP` otherwise, or injects a dummy `ORDER BY 1`. | Pre-12c ROWNUM emulation: limit-only wraps query in `SELECT * FROM (...) WHERE ROWNUM <= {0}`; limit+offset wraps twice with an inner `ROWNUM rn` and outer `WHERE rn > offset AND ROWNUM <= limit`. No native `OFFSET/FETCH` used. |
| Boolean literal | `'true'`/`'false'` (`serialize()` override) | raw `1`/`0` (no override, base default) | raw `1`/`0` (no override) | raw `1`/`0` (no override; `BIT` mapped to `Types.BOOLEAN`) | raw `1`/`0` (no override; no native BOOLEAN type) |
| String concat | `{0} || {1}` (ANSI base, unmodified) | `concat({0}, {1})` (function form) | `{0} || {1}` (ANSI base, unmodified) | `{0} + {1}` (T-SQL operator override) | `{0} || {1}` (ANSI base, unmodified) |
| CAST target names | base `getTypeNameForCode` (JDBC-driven) | `signed` (ints), `decimal` (floats), `char` (varchar) | base | base (uses `getTypeNameForCode` via registered `addTypeNameToCode` map) | `varchar(4000 char)`, `double precision` |
| Upsert/MERGE | No ON CONFLICT helper found; ANSI `MERGE` emulation only (`nativeMerge=false`) | `INSERT ... ON DUPLICATE KEY UPDATE` via `MySQLQueryFactory` helper (query-factory string append, not a `Templates` knob); also has `MySQLReplaceClause` (`REPLACE INTO`) | ANSI `MERGE` emulation only | ANSI `MERGE` emulation only | ANSI `MERGE` emulation only; bulk insert uses Oracle's `INSERT ALL ... SELECT * FROM dual` form (`serializeInsert` override) |
| RETURNING | Not modeled (JDBC generated-keys only, all dialects) | same | same | same | same |
| Window functions | Same base `SQLOps` window-function template set (`row_number()`, `rank()`, `lag()`, `lead()`, `ntile()`, ...) applies uniformly; no per-dialect gating/capability boolean exists in this layer | same | same | same | same (also uses `ROWNUM`, a non-window pseudo-column, for pagination specifically, separate from the ANSI window functions) |
| CTE (`WITH`) | base `"with "` / `"with recursive "`, unmodified | unmodified | unmodified | unmodified | `withRecursive` overridden to `"with "` (Oracle's recursive CTE syntax doesn't use the `RECURSIVE` keyword) |
| Null ordering | Native `NULLS FIRST`/`NULLS LAST` kept | Unsupported: `nullsFirst`/`nullsLast` set `null`, emulated with `CASE WHEN x IS NULL THEN 0/1 ELSE 1/0 END, x ASC/DESC` | Unsupported, same CASE-WHEN emulation | Unsupported (all three subclasses), same CASE-WHEN emulation | Native `NULLS FIRST`/`NULLS LAST` kept |
| `LIKE` escape | Drops the `ESCAPE '...'` clause entirely for `\\`-escape (native default), and case-insensitive `LIKE`/`STARTS_WITH`/... remapped to `ILIKE` | Drops `ESCAPE '...'` clause for `\\`-escape; case-insensitive ops emulated via lower-casing both sides (`{0l} like {%%1}`), no native `ILIKE` | Inherits SQL-standard `LIKE ... ESCAPE '\\'` (no override) | Inherits SQL-standard `LIKE ... ESCAPE '\\'`; regex `MATCHES` is faked as a plain `LIKE` (no real regex operator modeled) | Inherits SQL-standard `LIKE ... ESCAPE '\\'`; `MATCHES` maps to `regexp_like({0},{1})` |
| IN-list limit | none (`listMaxSize=0`) | none | none | none | `1000`, auto-partitioned into `OR`'d `IN` groups above that |
| Precedence retuning | Re-levels `IS NULL`, `CONCAT`, `MATCHES`, `IN`, `BETWEEN`, `LIKE*`, comparisons, `EQ` into a fine-grained ladder just below/around `COMPARISON` | Re-levels `EQ`/`EQ_IGNORE_CASE`/`NE` to `COMPARISON`, `BETWEEN` to `CASE` | Re-levels `<,>,<=,>=` and `EQ/NE` around `COMPARISON` | Re-levels `NEGATE` to `ARITH_LOW`; pushes `BETWEEN/IN/NOT_IN/LIKE*` all the way to `OR`/`OR+1` (SQL Server needs much more aggressive parenthesization around these in this model) | Re-levels `EQ/NE` to `COMPARISON`; pushes `IS NULL/LIKE*/BETWEEN/IN/EXISTS` to `COMPARISON+1` |

## 5. Design lessons and Rust port recommendation

### 5.1 What the Java design gets right

1. **Separation of "what" from "how many precedence levels".** `Template` (structure: which arg goes where, is it a string literal or a nested expression) is decoupled from `Templates`' precedence map. A dialect can override just precedence (`setPrecedence`) without touching the template string, or just the template without touching precedence (`add(op, pattern)` keeps inherited/default precedence via the two-arg overload only re-adding the map entry if absent).
2. **Templates are genuinely data, not code**, for the ~90% case: `add(Ops.X, "{0} op {1}", precedenceInt)`. This makes the operator table auditable at a glance (see the tables above) and trivially diffable between dialects.
3. **But pagination, MERGE, bulk INSERT, and null ordering are not data** - they are method overrides (`serialize(...)`, `serializeModifiers(...)`, `serializeInsert(...)`, `handleOrderBy(...)`). These require restructuring the query (wrapping in a subquery, injecting a synthetic `ROW_NUMBER()` projection column, rewriting `ORDER BY`), which a single-format-string-per-operator table cannot express. This is the load-bearing finding for the Rust port question below.
4. **Constant/expression duality in template transformers** (`toStartsWithViaLike` et al.) shows the pattern needs to special-case "I know this value now" (fold/escape immediately) vs "I'll only know this value at bind time" (build a `CONCAT` node instead) - and the Java code has a real, source-confirmed gap here (expression-side LIKE args are not escaped). A Rust port should decide this deliberately rather than copy the gap silently.
5. **RETURNING is intentionally out of scope** in this codebase - it relies on `Statement.RETURN_GENERATED_KEYS`. A Rust SQL DSL almost certainly wants native `RETURNING` (Postgres/SQLite/DuckDB support it natively and it is far more useful than driver-level generated-keys), so this is a place to explicitly diverge, not replicate.

### 5.2 Data-driven table vs. per-operator trait: comparison

**Option A - data-driven template table** (operator -> format string per dialect, parsed at runtime or macro-expanded at compile time, mirroring `Templates`/`TemplateFactory` closely):

- Pros: matches the existing design's biggest strength (auditable, diffable per-dialect tables, section 3/4 above translate almost directly into Rust `match`/const tables); easy to add a new simple operator across all dialects in one line; easy to snapshot-test (render every operator for every dialect and diff against a golden file); non-Rust-programmers (or generated code from a spec) can extend it.
- Cons: string-template parsing at runtime has a real cost (regex match per distinct template, even if cached) that Rust's compile-time-oriented culture will find wasteful; a stringly-typed template format defers a whole class of errors (wrong arg index, mismatched arg count, invalid modifier combination) to runtime/test-time instead of compile-time; the "structural" ~10% (pagination rewrites, MERGE emulation, null-ordering fallback, IN-list partitioning) still needs an escape hatch into real code no matter what, so a pure data table is never sufficient alone - it must be Option A *for the operator table* plus explicit Rust functions for query-shape rewrites (which the Java code already does via method overrides, so this isn't new complexity, just an acknowledged split).

**Option B - a trait with one method per operator** (`trait Dialect { fn eq(&self, l: Doc, r: Doc) -> Doc; fn like(&self, ...) -> Doc; ... }`):

- Pros: fully compile-time checked (a new operator added to the trait forces every dialect impl to handle it, no silent "falls through to base default" surprise, no risk of stringly-typed argument-index bugs); each method can freely mix "declarative concat" and "structural rewrite" logic without an artificial split between a data table and override methods, matching the reality that pagination et al. are already code, not data, in the Java source; better IDE navigation and inlining; no runtime template cache or parsing cost.
- Cons: ~150+ methods (matching the size of `Templates`'s ~150-entry `IdentityHashMap`, per its own initial-capacity hint) is a large trait surface; adding one new operator means editing N dialect impls (mitigated by default trait methods delegating to an "ANSI base" impl, which is exactly what `SQLTemplates extends Templates` already does via inheritance - the trait equivalent is a default-method-heavy base trait plus dialect structs that override only what differs, i.e. the same 5-15% override rate seen in the tables above); precedence/parenthesization logic must be threaded through explicitly (each method needs to know its own precedence and the precedence of whatever `Doc`/AST fragment its children produced) rather than being a single generic algorithm over a lookup table - this is more code but also more explicit, which aids debugging.

**Recommendation: a hybrid, weighted toward Option B (trait/methods) for the operator surface, with a small declarative table only for the genuinely uniform, leaf-level string-substitution operators (math functions, date parts, casts).**

Reasoning:

- The evidence in this repo is that the "pure data" model already isn't pure: `SQLTemplates` is ~1270 lines and a large fraction of the interesting dialect divergence (pagination, MERGE, null ordering fallback, bulk insert, forUpdate/forShare flag placement) lives in **method overrides**, not the template map. A Rust port that starts from "let's make everything a format-string table" will rediscover this and end up bolting a parallel trait/override system on top anyway - better to design that split up front.
- Rust's compile-time story is much stronger than Java's reflection-adjacent runtime template cache. A `match` over an `Operator` enum, or a trait method per operator family (comparisons, string ops, date ops, math ops, aggregate ops - mirroring the existing `Ops.StringOps`/`Ops.DateTimeOps`/`Ops.MathOps`/`Ops.AggOps` nested groupings, which already partition the ~150 operators into ~6 cohesive families of 10-40 each) keeps each family's trait small and gives far better error messages: a missing dialect override becomes "method not implemented" or "falls through to a documented default," found at compile time or via an exhaustiveness-style test, instead of `SerializerBase`'s current runtime `IllegalArgumentException` ("No pattern found for %s") thrown mid-query-build in strict mode (`SerializerBase.visitOperation`, `else if (strict)` branch) - a failure mode that is much later and much less discoverable than a Rust compiler error.
- Precedence-driven parenthesization is exactly the kind of small, generic, well-tested algorithm that *should* stay data-driven and centralized (one function, `(parent_precedence, child_precedence, is_first, same_precedence_set) -> needs_parens`, taking two `u8`/enum precedence values) - port `SerializerBase.visitOperation`'s parenthesization branch almost verbatim as a free function, and have every dialect's per-operator method return `(rendered_text, precedence_level)` (or a `Doc` type that carries its own precedence tag) so the shared function can wrap it. This avoids re-deriving parenthesization logic per dialect/operator, which is the main thing the pure-trait approach risks losing if each method hand-rolls its own paren logic.
- For simple, leaf, string-substitution-only operators with no structural rewrite needs (roughly the `MathOps`/`DateTimeOps` families - `abs({0})`, `sin({0})`, `dateadd('year', {1}, {0})` and friends, which are ~60% of the ~150 operators and never need pagination-style query restructuring), a small const table (`&[(Operator, &str)]` per dialect, format-string interpreted with a tiny fixed-arity substitution function, not a general regex parser) captures the exact same "auditable diff table" benefit Java gets from `Templates`, without paying for a general-purpose runtime template parser. This is the 60% where Option A's pros dominate and its cons (stringly-typed, runtime-checked) barely matter because the operators are leaves with no precedence subtlety beyond "wrap args in parens or not," which can be a single bool per table row instead of a full precedence integer.
- Net structure recommended: `trait Dialect` with per-family methods for comparisons/boolean logic/LIKE-family/pagination/MERGE/null-ordering/RETURNING (the ~40% with real structural or precedence divergence across dialects, matching sections 3-4's knob tables), each dialect struct implementing only the ~10-20% that actually differs from an `AnsiDialect` default (mirroring `SQLTemplates`'s own inheritance-by-exception pattern, where e.g. `SQLiteTemplates` only overrides ~15 of the ~150 registered operators); plus one small per-dialect const table for the uniform leaf operators. This keeps the compile-time safety and superior error messages of a trait for the parts of the Java source that are demonstrably not data (structural rewrites), while keeping a lightweight, diffable, low-ceremony table for the parts that demonstrably are just string substitution (the majority of `MathOps`/`DateTimeOps`).

## Unresolved questions

- Should the Rust port model `RETURNING` natively (recommended, since target dialects like Postgres/SQLite likely matter more than this Java fork's JDBC-generated-keys-only design)? Confirmed absent in the Java source either way, so there is no existing behavior to match here - a fresh design decision.
- Should the Rust port intentionally fix the "LIKE escaping only applies to constant operands, not dynamic expression operands" gap identified in `TemplateFactory`'s `Transformed` functions, or intentionally mirror it for behavioral parity with existing QueryDSL-generated SQL? This affects correctness of user-supplied dynamic `contains()`/`startsWith()` values against columns containing `%`/`_`.
- The 1000-element Oracle `listMaxSize` auto-partitioning (`Ops.IN`/`Ops.NOT_IN` handling in `SQLSerializer.visitOperation`) rewrites a single `IN` into `OR`-joined groups at serialization time; worth deciding whether the Rust port keeps this as an automatic serializer behavior (source of surprising SQL shape changes based on collection size) or exposes it as an explicit user-facing chunking API instead.

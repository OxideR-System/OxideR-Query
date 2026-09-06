# QueryDSL operator catalog and typed expression hierarchy

Scope: `com.querydsl.core.types` and `com.querydsl.core.types.dsl` in
`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/`.
All paths below are relative to the repo root
`/c/Users/Admin/Workspaces/OxideR/querydsl` unless stated otherwise.
Cross-references into `querydsl-sql` and `querydsl-jpa` are called out explicitly since several
`Ops` entries have zero DSL exposure in core and only become reachable from those modules.

Legend of file paths for exposing classes, cited once here, referenced by short name in tables:

- `Ops` = `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/Ops.java`
- `Operator`/`Operation`/`Expression`/`Visitor`/`Predicate` = same dir, `Operator.java`, `Operation.java`, `Expression.java`, `Visitor.java`, `Predicate.java`
- `ExpressionUtils` = `.../types/ExpressionUtils.java`
- `Templates` / `JavaTemplates` = `.../types/Templates.java`, `.../types/JavaTemplates.java`
- `PathType` = `.../types/PathType.java`
- `OperationImpl` / `PredicateOperation` / `ConstantImpl` / `NullExpression` = `.../types/*.java`
- All `dsl` classes (SimpleExpression, ComparableExpressionBase, ComparableExpression, NumberExpression,
  StringExpression, BooleanExpression, DateExpression, TimeExpression, DateTimeExpression,
  TemporalExpression, EnumExpression, CollectionExpressionBase, ArrayExpression, ArrayPath,
  MapExpressionBase, ListExpression, Expressions, MathExpressions, StringExpressions, Coalesce,
  CaseBuilder, CaseForEqBuilder, DslExpression, LiteralExpression, NumberOperation, BooleanOperation,
  StringOperation, Wildcard) = `.../types/dsl/<ClassName>.java`
- `SQLExpressions` (querydsl-sql module) = `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/SQLExpressions.java`
- `JPAExpressions` (querydsl-jpa module) = `querydsl-libraries/querydsl-jpa/src/main/java/com/querydsl/jpa/JPAExpressions.java`
- `BeanPath` = `.../types/dsl/BeanPath.java`

---

## 1. Complete operator catalog

`Ops` (main enum, 72 entries) plus five nested enums: `AggOps` (10), `QuantOps` (5), `DateTimeOps` (34),
`MathOps` (28), `StringOps` (10). Total 159 operators.
Every operator's declared `Class<?> getType()` is its abstract result-type ceiling (e.g. `Number.class`,
`Boolean.class`) - concrete DSL factory calls narrow this to the exact `T` at construction time (see
section 3).

A "no DSL exposure" operator means: it exists in `Ops`/`Templates`, has a serialization pattern for at
least one backend, but no `querydsl-core` DSL method or `querydsl-sql`/`querydsl-jpa` static factory
builds it directly. It is reachable only via raw `Expressions.operation(...)`/`ExpressionUtils.operation(...)`
calls or custom templates.

### 1.1 Comparison / equality

| Operator | Arity | Result | Exposing class(es) | Method(s) | Arg / return types |
|---|---|---|---|---|---|
| EQ | 2 | Boolean | SimpleExpression | `eq(T)`, `eq(Expression<? super T>)` | `eq(null)` throws `IllegalArgumentException` - use `isNull()` |
| NE | 2 | Boolean | SimpleExpression | `ne(T)`, `ne(Expression<? super T>)` | `ne(null)` throws `IllegalArgumentException` - use `isNotNull()` |
| IS_NULL | 1 | Boolean | SimpleExpression | `isNull()` | cached per-instance field |
| IS_NOT_NULL | 1 | Boolean | SimpleExpression | `isNotNull()` | cached per-instance field |
| INSTANCE_OF | 2 | Boolean | BeanPath | `instanceOf(Class<B>)` | entity-path only, not on scalar `SimpleExpression` |
| GOE | 2 | Boolean | ComparableExpression, NumberExpression (own impl) | `goe(T)`, `goe(Expression<T>)`, `+goeAll/goeAny` | see section 3 for why Number reimplements this |
| GT | 2 | Boolean | ComparableExpression, NumberExpression (own impl) | `gt(T)`, `gt(Expression<T>)`, `+gtAll/gtAny` | quantified variants take `CollectionExpression` or `SubQueryExpression` |
| LOE | 2 | Boolean | ComparableExpression, NumberExpression (own impl) | `loe(T)`, `loe(Expression<T>)`, `+loeAll/loeAny` | |
| LT | 2 | Boolean | ComparableExpression, NumberExpression (own impl) | `lt(T)`, `lt(Expression<T>)`, `+ltAll/ltAny` | NumberExpression only has Collection-flavored `ltAll/ltAny`/`loeAll/loeAny`, no SubQuery overload (asymmetric vs `gt`/`goe`) |
| BETWEEN | 3 | Boolean | ComparableExpression, NumberExpression (own impl) | `between(T,T)`, `between(Expr,Expr)`, `notBetween(...)` | one-sided null bound degrades to `GOE`/`LOE`; both-null throws `IllegalArgumentException` |
| EQ_IGNORE_CASE | 2 | Boolean | StringExpression | `equalsIgnoreCase(String\|Expression<String>)` | |

### 1.2 Boolean logic

| Operator | Arity | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|---|
| AND | 2 | Boolean | BooleanExpression, ExpressionUtils | `and(Predicate)`, `andAnyOf(Predicate...)`, `ExpressionUtils.and(Predicate,Predicate)` | null-safe: a null operand (after `extract()`) makes `and` return the other operand unchanged |
| OR | 2 | Boolean | BooleanExpression, ExpressionUtils | `or(Predicate)`, `orAllOf(Predicate...)`, `ExpressionUtils.or(...)` | same null-safety as AND |
| NOT | 1 | Boolean | BooleanExpression (`Predicate.not()`) | `not()` | `BooleanOperation.not()` special-cases `not(not(x)) -> x` (double-negation elimination) |
| XOR | 2 | Boolean | **no DSL exposure** | - | `Templates`/`JavaTemplates` define syntax only; unreachable except via raw `Expressions.operation(Boolean.class, Ops.XOR, ...)` |
| XNOR | 2 | Boolean | **no DSL exposure** | - | same as XOR |

### 1.3 String

| Operator | Arity | Result | Exposing class(es) | Method(s) | Arg / return types |
|---|---|---|---|---|---|
| CHAR_AT | 2 | Character (via `SimpleExpression<Character>`) | StringExpression | `charAt(int\|Expression<Integer>)` | |
| CONCAT | 2 | String | StringExpression | `append/concat(String\|Expression<String>)`, `prepend(...)` | `append`=`concat` alias; `prepend` swaps arg order |
| LOWER | 1 | String | StringExpression | `lower()`, `toLowerCase()` | cached |
| UPPER | 1 | String | StringExpression | `upper()`, `toUpperCase()` | cached |
| TRIM | 1 | String | StringExpression | `trim()` | cached; whitespace only, no char arg (cf. `StringOps.LTRIM/RTRIM`) |
| SUBSTR_1ARG | 2 | String | StringExpression | `substring(int\|Expression<Integer>)` | |
| SUBSTR_2ARGS | 3 | String | StringExpression | `substring(begin, end)` (4 overloads mixing int/Expression) | |
| MATCHES | 2 | Boolean | StringExpression | `matches(String\|Expression<String>)` | regex; some backends (JPA) convert to LIKE and may throw if unconvertible |
| MATCHES_IC | 2 | Boolean | **no DSL exposure** | - | template + backend serializer special-casing only (JPQLSerializer, MongodbSerializer); no builder method anywhere in core |
| STRING_LENGTH | 1 | Integer | StringExpression | `length()` | cached |
| STRING_IS_EMPTY | 1 | Boolean | StringExpression | `isEmpty()`, `isNotEmpty()=isEmpty().not()` | cached |
| STARTS_WITH | 2 | Boolean | StringExpression | `startsWith(String\|Expression<String>)` | |
| STARTS_WITH_IC | 2 | Boolean | StringExpression | `startsWithIgnoreCase(...)` | |
| INDEX_OF | 2 | Integer | StringExpression | `indexOf(String\|Expression<String>)` | |
| INDEX_OF_2ARGS | 3 | Integer | StringExpression | `indexOf(str, int i)` | |
| ENDS_WITH | 2 | Boolean | StringExpression | `endsWith(...)` | |
| ENDS_WITH_IC | 2 | Boolean | StringExpression | `endsWithIgnoreCase(...)` | |
| STRING_CONTAINS | 2 | Boolean | StringExpression | `contains(String\|Expression<String>)` | |
| STRING_CONTAINS_IC | 2 | Boolean | StringExpression | `containsIgnoreCase(...)` | |
| LIKE | 2 | Boolean | StringExpression, NumberExpression | `like(String\|Expression<String>)` | NumberExpression also exposes `like()` by first casting itself with `stringValue()` |
| LIKE_IC | 2 | Boolean | StringExpression | `likeIgnoreCase(...)` | |
| LIKE_ESCAPE | 3 | Boolean | StringExpression | `like(str, char escape)`, `notLike(str, escape)` | |
| LIKE_ESCAPE_IC | 3 | Boolean | StringExpression | `likeIgnoreCase(str, char escape)` | |
| STRING_CAST | 1 | String | LiteralExpression, NumberExpression (own impl) | `stringValue()` | `StringExpression.stringValue()` overridden to return `this` (identity) |
| StringOps.LEFT | 2 | String | **SQL module only** (`SQLExpressions.left`) | - | no core exposure |
| StringOps.RIGHT | 2 | String | **SQL module only** (`SQLExpressions.right`) | - | no core exposure |
| StringOps.LTRIM | 1 | String | StringExpressions | `ltrim(Expression<String>)` | static-only |
| StringOps.RTRIM | 1 | String | StringExpressions | `rtrim(Expression<String>)` | static-only |
| StringOps.LPAD | 2 | String | StringExpressions | `lpad(in, int\|Expression<Integer> length)` | static-only |
| StringOps.RPAD | 2 | String | StringExpressions | `rpad(in, length)` | static-only |
| StringOps.LPAD2 | 3 | String | StringExpressions | `lpad(in, length, char c)` | static-only |
| StringOps.RPAD2 | 3 | String | StringExpressions | `rpad(in, length, char c)` | static-only |
| StringOps.LOCATE | 2 | Number (Integer) | StringExpression | `locate(String\|Expression<String>)` | instance method, arg order `(str, this)` in generated AST |
| StringOps.LOCATE2 | 3 | Number (Integer) | StringExpression | `locate(str, start)` | 3 overloads mixing int/Expression |

### 1.4 Numeric / arithmetic (Ops main enum)

| Operator | Arity | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|---|
| NEGATE | 1 | Number | NumberExpression | `negate()` | cached; `NumberOperation.negate()` special-cases `negate(negate(x)) -> x` |
| ADD | 2 | Number | NumberExpression | `add(N\|Expression<N>)` | result type is fixed to `this.getType()`, not promoted (see section 3) |
| SUB | 2 | Number | NumberExpression | `subtract(N\|Expression<N>)` | |
| MULT | 2 | Number | NumberExpression | `multiply(N\|Expression<N>)` | |
| DIV | 2 | Number | NumberExpression | `divide(N\|Expression<N>)` | result type is `Double` if operand types differ, else `this.getType()` (`getDivisionType`) |
| MOD | 2 | Number | NumberExpression | `mod(T\|Expression<T>)` | operand type fixed to `T`, not generic `N` like add/sub/mult/div |
| NUMCAST | 2 | Number | LiteralExpression, NumberExpression (own impl) | `castToNum(Class<A>)`, plus `byteValue/shortValue/intValue/longValue/floatValue/doubleValue()` sugar | `NumberExpression.castToNum` short-circuits to `this` if `type.equals(getType())` |

### 1.5 Math functions (`Ops.MathOps`, 28 entries)

| Operator | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|
| ABS | Number (T) | NumberExpression | `abs()` | instance, cached |
| CEIL | Number (T) | NumberExpression | `ceil()` | instance, cached |
| FLOOR | Number (T) | NumberExpression | `floor()` | instance, cached |
| SQRT | Number (Double) | NumberExpression | `sqrt()` | instance, cached |
| ROUND | Number (T) | NumberExpression, MathExpressions | `round()` (instance) / `round(Expression)` (static, delegates to same op) | |
| ROUND2 | Number (T) | MathExpressions | `round(Expression, int scale)` | static-only, 2-arg |
| RANDOM | Number (Double) | NumberExpression, MathExpressions | `NumberExpression.random()` (static singleton constant), `MathExpressions.random()` (delegates) | |
| RANDOM2 | Number (Double) | MathExpressions | `random(int seed)` | static-only |
| MIN | Number (A) | NumberExpression, MathExpressions | `NumberExpression.min(Expression,Expression)` (static, binary min-of-two) | **not the same operator as the `AggOps.MIN_AGG` instance `.min()` aggregate** - same method name, different Ops, disambiguated by static vs instance call |
| MAX | Number (A) | NumberExpression, MathExpressions | `NumberExpression.max(Expression,Expression)` (static, binary max-of-two) | same caveat as MIN vs `AggOps.MAX_AGG` |
| ACOS | Number (Double) | MathExpressions | `acos(Expression)` | static-only |
| ASIN | Number (Double) | MathExpressions | `asin(Expression)` | static-only |
| ATAN | Number (Double) | MathExpressions | `atan(Expression)` | static-only |
| COS | Number (Double) | MathExpressions | `cos(Expression)` | static-only |
| COSH | Number (Double) | MathExpressions | `cosh(Expression)` | static-only |
| COT | Number (Double) | MathExpressions | `cot(Expression)` | static-only |
| COTH | Number (Double) | MathExpressions | `coth(Expression)` | static-only |
| SIN | Number (Double) | MathExpressions | `sin(Expression)` | static-only |
| SINH | Number (Double) | MathExpressions | `sinh(Expression)` | static-only |
| TAN | Number (Double) | MathExpressions | `tan(Expression)` | static-only |
| TANH | Number (Double) | MathExpressions | `tanh(Expression)` | static-only |
| EXP | Number (Double) | MathExpressions | `exp(Expression)` | static-only |
| LN | Number (Double) | MathExpressions | `ln(Expression)` | static-only |
| LOG | Number (Double) | MathExpressions | `log(Expression, int base)` | static-only |
| POWER | Number (Double) | MathExpressions | `power(Expression, int exponent)` | static-only, exponent is `int` not `Expression` |
| DEG | Number (Double) | MathExpressions | `degrees(Expression)` | static-only |
| RAD | Number (Double) | MathExpressions | `radians(Expression)` | static-only |
| SIGN | Number (Integer) | MathExpressions | `sign(Expression)` | static-only |

Design split: only 6 math ops (ABS, CEIL, FLOOR, SQRT, ROUND, RANDOM) live as `NumberExpression` instance
methods; the other 22 are exposed exclusively as static functions on `MathExpressions`
(`types/dsl/MathExpressions.java`), whose Javadoc says "supported by the SQL module" - i.e. core defines
the operator + default template, but idiomatic usage assumes a SQL/JPA backend.

### 1.6 Date / time (`Ops.DateTimeOps`, 34 entries)

| Operator | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|
| CURRENT_DATE | Comparable | DateExpression, DateTimeExpression | `currentDate()` (static) | two independent static factories (Date-only vs DateTime-flavored) |
| CURRENT_TIME | Comparable | TimeExpression | `currentTime()` (static) | |
| CURRENT_TIMESTAMP | Comparable | DateTimeExpression | `currentTimestamp()` (static) | |
| DATE | Comparable | **SQL module only** (`SQLExpressions.date(DateTimeExpression)`) | - | truncates a datetime to a date |
| SYSDATE | Comparable | **no DSL exposure anywhere** | - | template string `"sysdate"` defined in core `Templates` and `JPQLTemplates`; no factory method found in any module |
| YEAR | Integer | DateExpression, DateTimeExpression | `year()` | cached |
| MONTH | Integer | DateExpression, DateTimeExpression | `month()` | cached |
| WEEK | Integer | DateExpression, DateTimeExpression | `week()` | cached |
| YEAR_MONTH | Integer | DateExpression, DateTimeExpression | `yearMonth()` | cached |
| YEAR_WEEK | Integer | DateExpression, DateTimeExpression | `yearWeek()` | cached, ISO year-week |
| DAY_OF_WEEK | Integer | DateExpression, DateTimeExpression | `dayOfWeek()` | cached; "not supported in JDOQL/Derby" per Javadoc |
| DAY_OF_MONTH | Integer | DateExpression, DateTimeExpression | `dayOfMonth()` | cached |
| DAY_OF_YEAR | Integer | DateExpression, DateTimeExpression | `dayOfYear()` | cached |
| HOUR | Integer | TimeExpression, DateTimeExpression | `hour()` | cached |
| MINUTE | Integer | TimeExpression, DateTimeExpression | `minute()` | cached |
| SECOND | Float | TimeExpression, DateTimeExpression | `second()` | cached; **note the `Float` result type**, unlike the other integer date parts |
| MILLISECOND | Integer | TimeExpression, DateTimeExpression | `milliSecond()` | cached; "always 0 in JPA/JDO modules" per Javadoc |
| ADD_YEARS | Comparable | **SQL module only** | `SQLExpressions.addYears(date, years)`, generic `dateadd(unit, date, amount)` | no core DSL method |
| ADD_MONTHS | Comparable | **SQL module only** | `SQLExpressions.addMonths(...)` | |
| ADD_WEEKS | Comparable | **SQL module only** | `SQLExpressions.addWeeks(...)` | |
| ADD_DAYS | Comparable | **SQL module only** | `SQLExpressions.addDays(...)` | |
| ADD_HOURS | Comparable | **SQL module only** | `SQLExpressions.addHours(...)` | |
| ADD_MINUTES | Comparable | **SQL module only** | `SQLExpressions.addMinutes(...)` | |
| ADD_SECONDS | Comparable | **SQL module only** | `SQLExpressions.addSeconds(...)` | millisecond variant explicitly `// TODO` (unimplemented) in `SQLExpressions` |
| DIFF_YEARS | Number (Integer) | **SQL module only** | `SQLExpressions.datediff(DatePart.year, start, end)` | generic `datediff(unit, ...)`, no per-unit convenience method |
| DIFF_MONTHS | Number (Integer) | **SQL module only** | `datediff(DatePart.month, ...)` | |
| DIFF_WEEKS | Number (Integer) | **SQL module only** | `datediff(DatePart.week, ...)` | |
| DIFF_DAYS | Number (Integer) | **SQL module only** | `datediff(DatePart.day, ...)` | |
| DIFF_HOURS | Number (Integer) | **SQL module only** | `datediff(DatePart.hour, ...)` | |
| DIFF_MINUTES | Number (Integer) | **SQL module only** | `datediff(DatePart.minute, ...)` | |
| DIFF_SECONDS | Number (Integer) | **SQL module only** | `datediff(DatePart.second, ...)` | millisecond variant `// TODO` |
| TRUNC_YEAR | Comparable | **SQL module only** | `SQLExpressions.datetrunc(DatePart.year, expr)` | |
| TRUNC_MONTH | Comparable | **SQL module only** | `datetrunc(DatePart.month, ...)` | |
| TRUNC_WEEK | Comparable | **SQL module only** | `datetrunc(DatePart.week, ...)` | |
| TRUNC_DAY | Comparable | **SQL module only** | `datetrunc(DatePart.day, ...)` | |
| TRUNC_HOUR | Comparable | **SQL module only** | `datetrunc(DatePart.hour, ...)` | |
| TRUNC_MINUTE | Comparable | **SQL module only** | `datetrunc(DatePart.minute, ...)` | |
| TRUNC_SECOND | Comparable | **SQL module only** | `datetrunc(DatePart.second, ...)` | |

Key finding: none of ADD_*/DIFF_*/TRUNC_*/DATE/SYSDATE are reachable from `querydsl-core` at all - the
whole "date arithmetic" surface of `DateTimeOps` is implemented in `querydsl-sql`'s `SQLExpressions`
(`querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/SQLExpressions.java`, lines ~49-620),
which maps a `DatePart` enum to the `Ops.DateTimeOps` constants via `DATE_ADD_OPS`/`DATE_DIFF_OPS`/`DATE_TRUNC_OPS`
lookup tables. `DateExpression`/`TimeExpression`/`DateTimeExpression` in core only expose field-extraction
(`year()`, `month()`, ...) and `currentX()`, never arithmetic.

### 1.7 Aggregate (`Ops.AggOps`, 10 entries)

| Operator | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|
| COUNT_AGG | Number (Long) | SimpleExpression | `count()` | cached, on every expression type |
| COUNT_DISTINCT_AGG | Number (Long) | SimpleExpression | `countDistinct()` | cached |
| COUNT_ALL_AGG | Number (Long/Integer) | Wildcard | `Wildcard.count`, `Wildcard.countAsInt` constants | `count(*)` |
| COUNT_DISTINCT_ALL_AGG | Number (Long) | Wildcard | `Wildcard.countDistinct` constant | `count(distinct *)` |
| MIN_AGG | Comparable | ComparableExpressionBase (+ typed overrides in ComparableExpression, NumberExpression, StringExpression, DateExpression, DateTimeExpression) | `min()` | cached per subclass |
| MAX_AGG | Comparable | same set as MIN_AGG | `max()` | cached per subclass |
| AVG_AGG | Number (Double) | NumberExpression | `avg()` | cached |
| SUM_AGG | Number | NumberExpression | `sumAggregate()`, plus type-specific `sumLong()/sumDouble()/sumBigDecimal()/sumBigInteger()` | `sum()` itself is private; public entry points force explicit numeric widening |
| BOOLEAN_ALL | Boolean | **SQL module only** | `SQLExpressions.all(BooleanExpression)` | SQL `bool_and`-style aggregate |
| BOOLEAN_ANY | Boolean | **SQL module only** | `SQLExpressions.any(BooleanExpression)` | SQL `bool_or`-style aggregate |

`Ops.aggOps` (`public static final Set<Ops.AggOps> aggOps`, `types/Ops.java` lines 141-149) is a
convenience classification set containing `{AVG_AGG, COUNT_AGG, COUNT_DISTINCT_AGG, MAX_AGG, MIN_AGG,
SUM_AGG}`, used by `querydsl-collections` (`CollQuerySerializer.java`, `DefaultQueryEngine.java`) to
detect aggregate projections during in-memory evaluation.

### 1.8 Collection / array / map

| Operator | Arity | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|---|
| IN | 2 | Boolean | SimpleExpression, CollectionExpressionBase, ExpressionUtils | `in(Collection\|T...\|CollectionExpression\|SubQueryExpression\|Expression...)`, `contains(E)` (reverse-argument-order sugar) | `in(single-element collection)` optimizes to `eq()`; args are always `(element, collection)` regardless of which side calls it |
| NOT_IN | 2 | Boolean | SimpleExpression, ExpressionUtils | `notIn(...)` (same overload set as `in`) | single-element optimizes to `ne()` |
| COL_IS_EMPTY | 1 | Boolean | CollectionExpressionBase | `isEmpty()`, `isNotEmpty()` | cached |
| COL_SIZE | 1 | Integer | CollectionExpressionBase | `size()` | cached |
| ARRAY_SIZE | 1 | Number (Integer) | ArrayPath | `size()` | cached; array *element access* (`get(index)`) is a `Path` (`PathType.ARRAYVALUE`), not an `Operation` - see section 4 |
| CONTAINS_KEY | 2 | Boolean | MapExpressionBase | `containsKey(K\|Expression<K>)` | |
| CONTAINS_VALUE | 2 | Boolean | MapExpressionBase | `containsValue(V\|Expression<V>)` | |
| MAP_SIZE | 1 | Integer | MapExpressionBase | `size()` | cached |
| MAP_IS_EMPTY | 1 | Boolean | MapExpressionBase | `isEmpty()`, `isNotEmpty()` | cached |

Map/List *value* access (`map.get(key)`, `list.get(index)`) is likewise not an `Ops` operator - it is
modeled with `PathType.MAPVALUE`/`MAPVALUE_CONSTANT`/`LISTVALUE`/`LISTVALUE_CONSTANT`, because the
result is itself an addressable `Path` that can be the root of further property navigation. `MapExpressionBase.contains(K,V)`
is pure sugar: `get(key).eq(value)`, not its own operator.

### 1.9 Quantifiers (`Ops.QuantOps`, 5 entries)

| Operator | Arity | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|---|
| ANY | 1 | Object (T) | ExpressionUtils | `ExpressionUtils.any(CollectionExpression\|SubQueryExpression)` | used internally by `SimpleExpression.eqAny/gtAny/goeAny/ltAny/loeAny` |
| ALL | 1 | Object (T) | ExpressionUtils | `ExpressionUtils.all(CollectionExpression\|SubQueryExpression)` | used internally by `...All` sibling methods |
| AVG_IN_COL | Number | **JPA module only** | `JPAExpressions.avgInCol(CollectionExpression)` | correlated-collection aggregate, e.g. `avg(elements(c.orders))` in HQL |
| MAX_IN_COL | Comparable | **JPA module only** | `JPAExpressions.maxInCol(...)` | |
| MIN_IN_COL | Comparable | **JPA module only** | `JPAExpressions.minInCol(...)` | |

`ANY`/`ALL` are not directly callable as top-level predicates; they only appear as the *right-hand side*
of an `EQ/NE/GT/GOE/LT/LOE` comparison built by the `xxxAny`/`xxxAll` family of methods across
`SimpleExpression`, `ComparableExpression`, and `NumberExpression`.

### 1.10 Case / coalesce / cast / subquery

| Operator | Arity | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|---|
| CASE | 2 (op, chain) | Object (T) | CaseBuilder | `new CaseBuilder().when(pred).then(x)...otherwise(y)` | wraps the whole `CASE_WHEN`/`CASE_ELSE` chain in one outer `CASE` operation |
| CASE_WHEN | 3 | Object (T) | CaseBuilder (internal) | n/a (not user-called directly) | `(condition, then, nestedElseOrWhen)`, right-folded, see section 4 |
| CASE_ELSE | 1 | Object (T) | CaseBuilder (internal) | n/a | innermost node of the chain |
| CASE_EQ | 2 | Object (T) | CaseForEqBuilder (via `SimpleExpression.when(...)`) | `expr.when(v1).then(t1).when(v2).then(t2)...otherwise(d)` | equality-switch sugar; wraps `(base, chain)` |
| CASE_EQ_WHEN | 4 | Object (T) | CaseForEqBuilder (internal) | n/a | `(base, eqValue, then, nestedChain)` |
| CASE_EQ_ELSE | 1 | Object (T) | CaseForEqBuilder (internal) | n/a | innermost node |
| COALESCE | 1 (wraps a LIST arg) | Object (T) | Coalesce (via `ComparableExpressionBase.coalesce(...)` and per-type overrides) | `expr.coalesce(other, ...)`, `new Coalesce<>(type, exprs...)` | single argument is itself an `Ops.LIST`/`SINGLETON` chain, not N direct args - see section 4 |
| NULLIF | 2 | Object (T) | SimpleExpression, ComparableExpressionBase (+ typed overrides) | `nullif(T\|Expression<T>)` | |
| EXISTS | 1 | Boolean | `FetchableSubQueryBase`/`ReactiveFetchableSubQueryBase` (`querydsl-core/.../support/`) | `subQuery.exists()` | only on subqueries, never on scalar expressions |

### 1.11 Misc / structural

| Operator | Arity | Result | Exposing class(es) | Method(s) | Notes |
|---|---|---|---|---|---|
| ALIAS | 2 | same as source | DslExpression and every typed subclass (`SimpleExpression`, `ComparableExpression`, `NumberExpression`, `StringExpression`, `BooleanExpression`, `DateExpression`, `DateTimeExpression`, `TimeExpression`, `EnumExpression`, `CollectionExpressionBase`, `Coalesce`) | `as(Path<T>)`, `as(String)` | every level of the hierarchy re-overrides `as()` purely to narrow the covariant return type - a Java-only ceremony (see section 3) |
| LIST | 2 | Object[]/Tuple | ExpressionUtils, Expressions | `Expressions.list(...)`, internal to `Coalesce`/`SimpleExpression.notIn(Expression...)` | builds a right-growing binary chain of `Ops.LIST` nodes, one per extra element |
| SET | 2 | Object[]/Tuple | Expressions | `Expressions.set(...)`, internal to `SimpleExpression.in(Expression...)` | same shape as LIST but semantically a set |
| SINGLETON | 2 | Object[]/Tuple | ExpressionUtils (internal) | n/a | used when a `LIST`/`SET` builder receives exactly one element, to keep it wrapped consistently |
| ORDINAL | 1 | Integer | EnumExpression | `ordinal()` | cached |
| WRAPPED | 1 | same as source | **no DSL exposure** | n/a | forces literal parenthesization `({0})` in generated SQL/JPQL text; internal serializer use only |
| ORDER | 1 | Object | ExpressionUtils (internal) | `ExpressionUtils.orderBy(List<OrderSpecifier<?>>)` | not a DSL-facing operator; `OrderSpecifier`/`ComparableExpressionBase.asc()/desc()` are the real user surface and don't go through this Ops entry at expression-build time |
| STRING_CAST | see 1.3 | | | | |
| NUMCAST | see 1.4 | | | | |

`PathType` (`types/PathType.java`) is a sibling `Operator` implementation (11 entries: `PROPERTY`,
`VARIABLE`, `DELEGATE`, `COLLECTION_ANY`, `LISTVALUE`, `LISTVALUE_CONSTANT`, `MAPVALUE`,
`MAPVALUE_CONSTANT`, `ARRAYVALUE`, `ARRAYVALUE_CONSTANT`, `TREATED_PATH`, `LIST_FIRST`) sharing the same
`Templates` map as `Ops`. It is not part of `Ops.java` and is out of this report's literal scope, but it
matters for section 4 because it is how indexed/keyed/property access is modeled - as a `Path` variant,
never as an `Operation`.

`Ops.compareOps`, `Ops.equalsOps`, `Ops.notEqualsOps` (`types/Ops.java` lines 134-139) are public static
classification `Set`s (`{EQ,NE,LT,GT,GOE,LOE}`, `{EQ}`, `{NE}`) with **zero usages anywhere in the
repository** (verified by repo-wide grep) - dead/vestigial public API, presumably kept for external
consumers or historical compatibility. Do not treat their existence as evidence of an actively used
classification mechanism.

---

## 2. Expression class hierarchy

```
Expression<T>                                    (types/Expression.java, interface)
 └─ DslExpression<T>                              (types/dsl/DslExpression.java, abstract)
     │  adds: as(Path<T>), as(String); wraps a plain `mixin: Expression<T>`; equals/hashCode/toString delegate to mixin
     │
     ├─ SimpleExpression<T>                       (types/dsl/SimpleExpression.java, abstract)
     │  │  adds: as(...) [narrowed], isNull(), isNotNull(), count(), countDistinct(),
     │  │        eq(T|Expr), eqAll/eqAny(CollectionExpression|SubQueryExpression),
     │  │        in(Collection|T...|CollectionExpression|SubQueryExpression|Expression...),
     │  │        ne(T|Expr), neAll/neAny(...), notIn(...),
     │  │        nullif(T|Expr), when(T|Expr) -> CaseForEqBuilder<T>
     │  │
     │  ├─ ComparableExpressionBase<T extends Comparable>   (ComparableExpressionBase.java, abstract)
     │  │  │  adds: asc(), desc() -> OrderSpecifier<T>, coalesce(...) x4 overloads,
     │  │  │        nullif(...) [narrowed], min(), max() [AggOps]
     │  │  │
     │  │  ├─ ComparableExpression<T extends Comparable>    (ComparableExpression.java, abstract)
     │  │  │  │  adds: between/notBetween(T|Expr, T|Expr), gt/goe/lt/loe(T|Expr) + xxxAll/xxxAny
     │  │  │  │        (CollectionExpression and SubQueryExpression variants for all four),
     │  │  │  │        min()/max() [narrowed return type], nullif/coalesce [narrowed]
     │  │  │  │
     │  │  │  └─ LiteralExpression<T extends Comparable>    (LiteralExpression.java, abstract)
     │  │  │     │  adds: castToNum(Class<A>), stringValue()
     │  │  │     │
     │  │  │     ├─ StringExpression                        (StringExpression.java, abstract)
     │  │  │     │     adds: append/concat/prepend, charAt, contains/containsIgnoreCase,
     │  │  │     │       endsWith(IC), equalsIgnoreCase, notEqualsIgnoreCase, indexOf(x2),
     │  │  │     │       isEmpty/isNotEmpty, length, like/likeIgnoreCase(+escape variants),
     │  │  │     │       notLike, locate(x2), lower/upper/toLowerCase/toUpperCase, matches,
     │  │  │     │       startsWith(IC), substring(x4 overloads), trim, min()/max() [narrowed],
     │  │  │     │       stringValue() overridden to identity
     │  │  │     │
     │  │  │     ├─ BooleanExpression implements Predicate   (BooleanExpression.java, abstract)
     │  │  │     │     adds: and/or(Predicate), andAnyOf/orAllOf(Predicate...), not() [Predicate],
     │  │  │     │       isTrue/isFalse, eq(Boolean) [cached true/false variants]
     │  │  │     │     NOTE: inherits gt/lt/goe/loe/between from ComparableExpression because
     │  │  │     │       java.lang.Boolean implements Comparable<Boolean> - questionable but legal (section 3)
     │  │  │     │
     │  │  │     ├─ EnumExpression<T extends Enum<T>>        (EnumExpression.java, abstract)
     │  │  │     │     adds: ordinal()
     │  │  │     │     NOTE: also inherits gt/lt/between from ComparableExpression (enum's natural
     │  │  │     │       ordinal-based Comparable) - usually meaningful, but not guaranteed portable across backends
     │  │  │     │
     │  │  │     └─ TemporalExpression<T extends Comparable> (TemporalExpression.java, abstract)
     │  │  │        │  adds: after(T|Expr) [=gt], before(T|Expr) [=lt]
     │  │  │        │
     │  │  │        ├─ DateExpression<T extends Comparable>     (DateExpression.java, abstract)
     │  │  │        │     adds: dayOfMonth/dayOfWeek/dayOfYear/month/week/year/yearMonth/yearWeek,
     │  │  │        │       static currentDate(), min()/max() [narrowed]
     │  │  │        │
     │  │  │        ├─ TimeExpression<T extends Comparable>    (TimeExpression.java, abstract)
     │  │  │        │     adds: hour/minute/second/milliSecond, static currentTime()
     │  │  │        │
     │  │  │        └─ DateTimeExpression<T extends Comparable> (DateTimeExpression.java, abstract)
     │  │  │              adds: union of DateExpression's + TimeExpression's field accessors,
     │  │  │                static currentDate()/currentTimestamp(), min()/max() [narrowed]
     │  │  │              NOTE: does not extend DateExpression or TimeExpression - duplicates
     │  │  │                their field-accessor methods instead of sharing code (section 3)
     │  │  │
     │  │  └─ (no other direct ComparableExpression subclasses in core)
     │  │
     │  └─ NumberExpression<T extends Number & Comparable<?>>  (NumberExpression.java, abstract)
     │        extends ComparableExpressionBase directly, NOT ComparableExpression/LiteralExpression
     │        adds (own, non-inherited): gt/goe/lt/loe/between/notBetween (own generic
     │          <A extends Number & Comparable<?>> parameterization, cross-numeric-type-aware),
     │          add/subtract/multiply/divide/mod, negate, abs/ceil/floor/round/sqrt,
     │          avg/sumAggregate/sumLong/sumDouble/sumBigDecimal/sumBigInteger,
     │          byteValue/shortValue/intValue/longValue/floatValue/doubleValue/castToNum,
     │          stringValue(), like(String|Expr) [via stringValue()],
     │          static random(), static max(Expr,Expr)/min(Expr,Expr) [binary MathOps, not aggregate],
     │          in/notIn(Number...) [narrowed, casts each element to T]
     │
     ├─ CollectionExpressionBase<T extends Collection<E>, E> implements CollectionExpression<T,E>
     │  │    (CollectionExpressionBase.java, abstract)
     │  │  adds: as(EntityPath<E>), contains(E|Expression<E>), isEmpty/isNotEmpty, size(), getElementType()
     │  │
     │  ├─ ListExpression<E,Q> interface extends CollectionExpression<List<E>,E>   (ListExpression.java)
     │  │     adds: get(int|Expression<Integer>) -> Q   [Path-based, not an Operation - see section 1.8]
     │  │
     │  └─ (SetPath, ListPath, CollectionPath extend CollectionPathBase/CollectionExpressionBase; not
     │      themselves part of the "typed expression" surface examined here)
     │
     ├─ MapExpressionBase<K,V,Q> implements MapExpression<K,V>   (MapExpressionBase.java, abstract)
     │     adds: contains(K,V), containsKey/containsValue, get(K|Expression<K>) [abstract, Path-based],
     │       isEmpty/isNotEmpty, size()
     │
     └─ ArrayExpression<A,T> interface                            (ArrayExpression.java)
           adds: size(), get(int|Expression<Integer>) -> SimpleExpression<T>   [Path-based]
           implemented by ArrayPath (ArrayPath.java)

Predicate extends Expression<Boolean>                              (types/Predicate.java, interface)
 └─ not() : Predicate   -- BooleanExpression implements this; PredicateOperation implements it too

Operation<T> extends Expression<T>                                 (types/Operation.java, interface)
 -- implemented by OperationImpl<T> (plain) and, in the `dsl` package, by a parallel
    "XxxOperation" class for every typed expression class (NumberOperation, StringOperation,
    BooleanOperation, ComparableOperation, DateOperation, DateTimeOperation, TimeOperation,
    EnumOperation, DslOperation, SimpleOperation). Each XxxOperation extends its matching
    XxxExpression AND implements Operation<T> by delegating getArg/getArgs/getOperator to an
    internal `opMixin: OperationImpl<T>` field. This is the "mixin" pattern described in section 3.

LiteralExpression's odd generic bound: `T extends Comparable` (raw type). NumberExpression's bound is
`T extends Number & Comparable<?>` - an intersection type LiteralExpression's declaration cannot express
without losing the `Number` half, which is why NumberExpression re-implements (rather than inherits)
castToNum/stringValue and does not sit under LiteralExpression at all.
```

Coalesce, CaseBuilder, and CaseForEqBuilder are AST-building helper classes, not `Expression`
subclasses (`Coalesce` extends `MutableExpressionBase<T>`, a mutable pre-`Expression` accumulator;
`CaseBuilder`/`CaseForEqBuilder` are plain builder objects with no `Expression` supertype at all) -
covered in section 4.

---

## 3. Type gating in Java, and where it leaks

### 3.1 Compile-time gating (the intended mechanism)

Each DSL class's generic bound is the primary gate:

- `SimpleExpression<T>` - no bound, so only `eq`/`ne`/`in`/`isNull`/`count`/alias are legal on *any* type.
- `ComparableExpressionBase<T extends Comparable>` - unlocks ordering-adjacent helpers (`asc/desc`,
  `min/max` aggregates, `coalesce`, `nullif`).
- `ComparableExpression<T extends Comparable>` - unlocks `gt/goe/lt/loe/between`.
- `NumberExpression<T extends Number & Comparable<?>>` - unlocks arithmetic (`add/subtract/multiply/divide/mod/negate`)
  and math functions, using an intersection bound so a caller can't get arithmetic on a merely-`Comparable`
  non-`Number` type.
- `StringExpression` - unlocks `like/contains/substring/...`.

This is exactly the "trait bound" pattern the task asks about, and maps directly onto Rust: each class
becomes a trait (`ComparableExpr: Expr`, `NumberExpr: ComparableExpr + Num`, `StringExpr: ComparableExpr`),
each generic bound becomes a `where` clause, and the compiler enforces the same legality rules with zero
runtime cost - stronger than Java, which still needs a handful of runtime checks (below) because type
erasure and the wrapper/mixin pattern can't fully encode the invariants statically.

Source: `types/dsl/ComparableExpressionBase.java`, `ComparableExpression.java`, `NumberExpression.java`,
`StringExpression.java` (class declarations, read directly).

### 3.2 Runtime gating (where Java's static types are not enough)

`OperationImpl`'s constructor (`types/OperationImpl.java` lines 43-51) does a runtime check:
`if (!operator.getType().isAssignableFrom(wrapped)) { throw new IllegalArgumentException(operator.name()); }`.
This means every `Ops` enum constant's declared `Class<?> getType()` (e.g. `Ops.ADD -> Number.class`,
`Ops.LIKE -> Boolean.class`) is *also* enforced at object-construction time, not only by the generic
method signatures that route callers to `Expressions.numberOperation`/`booleanOperation`/etc. This is
a safety net against someone calling `Expressions.operation(SomeUnrelatedType.class, Ops.ADD, ...)`
directly and bypassing the typed builder methods - the generic type system does not stop that call from
compiling (raw `Expressions.operation` takes `Class<? extends T>` + `Operator` + varargs `Expression<?>`,
with no static link between the two), so Querydsl backstops it with an `isAssignableFrom` check thrown
as an unchecked exception at runtime. A Rust port with an `Operator` enum whose result-type is fixed per
variant (not user-suppliable) removes this whole class of bug by construction.

Source: `types/OperationImpl.java` lines 39-51.

### 3.3 Places where Java's type system is looser than the domain semantics warrant

- **BooleanExpression inherits ordering comparisons.** `BooleanExpression extends LiteralExpression<Boolean>
  extends ComparableExpression<Boolean>`, and `java.lang.Boolean implements Comparable<Boolean>`, so
  `someBooleanExpr.gt(false)`, `.between(false, true)`, `.asc()` all compile and run, despite no SQL
  dialect meaningfully supporting `boolean_col > false` as an *ordering* comparison the way it does for
  numbers/strings/dates. Source: `types/dsl/BooleanExpression.java` class declaration; `LiteralExpression.java`.
  A Rust port should NOT give a `BoolExpr` trait the `Ord`-like comparison methods just because `bool: Ord`
  happens to hold in std - this is exactly the kind of accidental capability leak that a hand-designed
  trait hierarchy (rather than "inherit from Comparable-bound because the Java stdlib permits it") avoids.

- **EnumExpression inherits ordering comparisons too**, via the same `LiteralExpression<T extends
  Comparable>` path (`Enum<T> implements Comparable<T>` using ordinal order). This one is *sometimes*
  intentional (ordinal-based ordering is a real use case) but is inherited unconditionally rather than
  opted into, and its portability across SQL backends (which may store enums as strings) is unchecked at
  compile time.

- **`NumberExpression` re-implements rather than inherits `gt/lt/goe/loe/between`,** using its own
  `<A extends Number & Comparable<?>>` method-level type parameter, so that `intExpr.gt(5L)` (mixed
  Integer/Long comparison) compiles - something `ComparableExpression<Integer>.gt(Integer)` alone could
  not do. This is a deliberate, hand-rolled variance workaround; Rust generics with a `where T: PartialOrd<A>`
  style bound (or a small numeric-promotion trait) can express the same flexibility more directly and
  without duplicating ~15 methods verbatim (as `NumberExpression.java` does today).

- **Method-name overload collisions resolved only by static-vs-instance dispatch, not by the type system
  proper.** `NumberExpression.min(Expression,Expression)`/`.max(...)` are *static* factories for
  `MathOps.MIN`/`MAX` (binary "smaller/larger of two values"), while the *instance* `.min()`/`.max()`
  (no args) build `AggOps.MIN_AGG`/`MAX_AGG` (SQL aggregate over a column). These are unrelated operators
  that happen to share an English name; Java's overload resolution (arity + static/instance) keeps them
  apart, but nothing in the *type* signature communicates "these are semantically unrelated" - a
  transliterated Rust API should give these different names (e.g. `least`/`greatest` vs `min_agg`/`max_agg`)
  rather than relying on the same static/instance trick, which Rust does not really have (no true "static
  method on a value" ambiguity, but also no free binary `min`/`max` sitting next to an instance
  aggregate `.min()` without visual confusion).

- **The `OperationImpl` result-type parameter `Class<? extends T> type` is caller-supplied, not derived.**
  Every `Expressions.numberOperation(type, op, args...)` call trusts the caller's `type` argument (subject
  only to the coarse `Ops.ADD -> Number.class` assignability check from 3.2). Nothing prevents constructing
  `Expressions.numberOperation(BigDecimal.class, Ops.ADD, anIntPath, aStringConstant)` at compile time -
  the argument expressions' own types are never cross-checked against each other or against `type`. A Rust
  port with an AST node parameterized by a single concrete numeric type (chosen once, propagated at
  construction, and validated at the smart-constructor boundary) removes this whole surface. Source:
  `types/dsl/Expressions.java` (`numberOperation` family, lines ~851-863) and `types/OperationImpl.java`.

- **`as(Path<T>)`/`as(String)` are re-declared with a narrowed covariant return type at every single level
  of the hierarchy** (`DslExpression`, `SimpleExpression`, `ComparableExpression`, `NumberExpression`,
  `StringExpression`, `BooleanExpression`, `DateExpression`, `DateTimeExpression`, `TimeExpression`,
  `EnumExpression`) purely so that `numberExpr.as("x")` still yields a `NumberExpression`, not a
  `SimpleExpression`. This is ~10x duplicated boilerplate whose only purpose is working around Java's lack
  of a `Self` type. Rust's `Self` return type in a trait method makes every one of these overrides
  unnecessary.

---

## 4. Notable semantics

### 4.1 Null handling

`eq(null)`/`ne(null)` throw `IllegalArgumentException` at call time - the API forces the caller to use
`isNull()`/`isNotNull()` instead of silently generating `x = NULL` (which is always false/unknown in SQL
three-valued logic, a classic footgun). Source: `types/dsl/SimpleExpression.java` lines 125-131, 250-256.

`BooleanExpression.and(right)`/`.or(right)` accept a `@Nullable Predicate` and are null-safe: a null
`right` (after `ExpressionUtils.extract()` unwraps it) makes `and`/`or` degrade to returning `this`
unchanged rather than throwing or building a malformed operation. `ExpressionUtils.and/or(Predicate,
Predicate)` (the free-function form) is symmetric: a null on *either* side returns the other side. This is
what lets `BooleanBuilder`-style incremental predicate construction skip conditionally-absent clauses
without special-casing at every call site. Source: `types/dsl/BooleanExpression.java` lines 61-68, 106-113;
`types/ExpressionUtils.java` lines 325-335, 762-772.

`ExpressionUtils.extract(Expression)` (`types/ExpressionUtils.java` lines 805-819) is the mechanism behind
this null-safety: it fast-paths three known-canonical types (`PathImpl`, `PredicateOperation`,
`ConstantImpl`) as already-canonical, and otherwise dispatches `accept(ExtractorVisitor.DEFAULT, null)` to
unwrap any custom `Expression` wrapper (e.g. a `BooleanBuilder`) down to its real underlying node - `null`
in, `null` out.

`Comparable*.between(from, to)` treats a null bound as "open on that side": `between(null, to)` degrades to
`loe(to)`, `between(from, null)` degrades to `goe(from)`, and both-null throws `IllegalArgumentException`
("Either from or to needs to be non-null"). Source: `types/dsl/ComparableExpression.java` lines 61-74;
duplicated (not shared) in `types/dsl/NumberExpression.java` lines 409-422.

`CaseBuilder`/`CaseForEqBuilder` treat a null `otherwise(...)` value as `NullExpression.DEFAULT`
(`types/NullExpression.java`, a `TemplateExpressionImpl` that serializes as the literal `null`), so a
case expression's else-branch can be an explicit SQL `NULL` without special-casing at the call site.
Source: `types/dsl/CaseBuilder.java` lines 86-93, 96-99.

`ConstantImpl` interns small values (`Boolean` true/false, `Byte`/`Short`/`Integer`/`Long`/`Character` in
range `[0,256)`) as static final singletons (`types/ConstantImpl.java` lines 30-61), an allocation
optimization worth mirroring with e.g. a `once_cell`/const table in Rust if constant nodes are heap
allocated there too.

### 4.2 Ternary and n-ary operators

- `BETWEEN` (comparison) is genuinely ternary: `(subject, from, to)`.
- `LIKE_ESCAPE`/`LIKE_ESCAPE_IC` are ternary: `(subject, pattern, escapeChar)`.
- `SUBSTR_2ARGS` is ternary: `(subject, begin, end)`.
- `INDEX_OF_2ARGS`, `StringOps.LOCATE2` are ternary.
- `CASE_WHEN`/`CASE_EQ_WHEN` are effectively n-ary via right-folded nesting rather than a single flat
  N-argument operation - see 4.4.
- `COALESCE` is modeled as a **unary** `Operation` whose single argument is itself a right-growing chain
  of binary `Ops.LIST` (or a single `Ops.SINGLETON` wrapper for exactly one element) nodes - see 4.3. This
  is a deliberate choice: rather than growing `Operation.getArgs()` to arbitrary arity, `Ops.LIST`/`SET`
  chains push "collection of expressions" into a nested-binary-tree shape reusable across the `IN` operand,
  `COALESCE` argument list, and multi-column tuple construction (`Expressions.list(...)`, `.set(...)`).

### 4.3 How `Coalesce` builds its AST

`Coalesce<T>` (`types/dsl/Coalesce.java`) is a **mutable** accumulator, not an immutable `Expression`
itself - it extends `MutableExpressionBase<T>` and holds a plain `List<Expression<? extends T>> exprs`
that callers append to via `add(Expression<T>)`/`add(T constant)`. Finalization happens lazily and
per-target-type:

1. `getExpressionList()` calls `ExpressionUtils.list(getType(), exprs)`, which folds the accumulated list
   into either a single `Ops.SINGLETON` wrapper (exactly one element) or a right-growing chain of binary
   `Ops.LIST` nodes (`exprs[0] LIST exprs[1] LIST exprs[2] ...`), always as a *single* `Expression<T>`.
2. `getValue()`/`asBoolean()`/`asDate()`/`asDateTime()`/`asEnum()`/`asNumber()`/`asString()`/`asTime()`
   each call the matching `Expressions.xxxOperation(type, Ops.COALESCE, thatSingleListExpression)` -
   i.e. `COALESCE` is constructed as a **unary** operation whose one argument encodes the whole
   argument list, and the caller picks which typed wrapper class to receive back (there is no single
   canonical `Coalesce.build()`; the type is chosen by which `asX()` the caller invokes, which is itself
   driven by which typed `coalesce(...)` overload on `NumberExpression`/`StringExpression`/etc. dispatched
   into it). Source: `types/dsl/Coalesce.java` lines 65-143; call sites e.g.
   `types/dsl/NumberExpression.java` lines 861-913, `types/dsl/StringExpression.java` lines 883-932.
3. `Coalesce` is cache-invalidating: every `add(...)` call resets the memoized `value` field to `null`, so
   `getValue()` is idempotent-but-lazy, not eagerly rebuilt on every add.

### 4.4 How `CaseBuilder`/`CaseForEqBuilder` build their AST

Both builders accumulate `CaseElement`s (a `(predicateOrEqValue, targetExpression)` pair) into a `List`,
**always inserting at index 0** (`cases.add(0, element)`), so after `when(w1).then(t1).when(w2).then(t2)`
the list holds `[w2t2, w1t1]` - most-recently-added first. When `otherwise(elseExpr)` is finally called, it
prepends the else-clause the same way (`cases.add(0, CaseElement(null, elseExpr))`), producing
`[elseExpr, w2t2, w1t1]`, then folds that list **in forward iteration order** into a right-nested chain:

```
last = CASE_ELSE(elseExpr)                              // first iteration: the else element
last = CASE_WHEN(w2, t2, last)                           // second iteration
last = CASE_WHEN(w1, t1, last)                           // third iteration
result = CASE(last)                                      // outer wrap, once, in otherwise()
```

The net effect is a right fold that *preserves original insertion order for evaluation* (`w1` is checked
first, matching normal switch/case fallthrough semantics) despite the list itself being stored in reverse
insertion order internally. `CaseForEqBuilder` does the identical index-0-insert / forward-fold dance but
wraps each `CASE_EQ_WHEN` node with a `base` reference (the subject being compared) as an extra leading
argument: `CASE_EQ_WHEN(base, eqValue, then, nestedElseOrWhen)`, and the outer wrap is `CASE_EQ(base, chain)`
rather than a bare `CASE(chain)`. `CaseBuilder.Initial.then(Expression<A>)` also does runtime
`instanceof`-based type dispatch (`Predicate`, `StringExpression`, `NumberExpression`, `DateExpression`,
`DateTimeExpression`, `TimeExpression`, `ComparableExpression`, else generic) to pick which typed `Cases<A,Q>`
subclass (and therefore which typed result wrapper) the chain will ultimately produce - this is Java
substituting runtime type tests for what would be resolved by trait dispatch / pattern matching in Rust.
Source: `types/dsl/CaseBuilder.java` lines 79-117, 159-187; `types/dsl/CaseForEqBuilder.java` lines
78-106, 245-266, 325-344.

### 4.5 `eq`/`ne`/`in` accepting both constants and expressions

Every comparison-family method on `SimpleExpression` is duplicated as a `(T literal)` overload and an
`(Expression<...> other)` overload; the literal overload always just wraps via `ConstantImpl.create(...)`
and re-dispatches to the `Expression` overload - e.g. `eq(T right) { ...; return eq(ConstantImpl.create(right)); }`
(`types/dsl/SimpleExpression.java` lines 125-141). `in(...)` additionally special-cases a
single-element `Collection`/varargs array by degrading to `eq(...)`/`ne(...)` directly rather than
constructing a one-element `IN (...)` clause (`in(right.length==1) -> eq(right[0])`), a small but real
generated-SQL-shape optimization. `Collection`-based overloads route through `ConstantImpl.create(collection)`
(the whole collection becomes one constant node); `CollectionExpression`/`SubQueryExpression` overloads
pass the argument through unwrapped, since those are already `Expression`s, not literal values needing
wrapping. Source: `types/dsl/SimpleExpression.java` lines 117-345; `types/ExpressionUtils.java`
lines 405-467, 673-736 (the same pattern duplicated at the `ExpressionUtils` free-function level).

### 4.6 Operator precedence handling

`Templates` (`types/Templates.java`) maintains two parallel `IdentityHashMap<Operator,*>`s: one from
`Operator` to its rendering `Template` (a `{0} + {1}`-style pattern), one from `Operator` to an `int`
precedence. The nested `Precedence` class (lines 28-40) defines a fixed ladder modeled on Java's own
operator precedence: `HIGHEST=-1 < DOT=5 < NOT_HIGH=10 < NEGATE=20 < ARITH_HIGH=30 < ARITH_LOW=40 <
COMPARISON=50 < EQUALITY=60 < CASE=LIST=70 < NOT=80 < AND=90 < XOR=XNOR=100 < OR=110`. Only *binary infix*
operators that can legally nest ambiguously get an explicit precedence (`AND`, `OR`, `NOT`, `XOR`, `XNOR`,
`BETWEEN`, `GOE/GT/LOE/LT`, `NEGATE`, `ADD/DIV/MOD/MULT/SUB`, `EQ/EQ_IGNORE_CASE/NE`, `IN/NOT_IN/IS_NULL/
IS_NOT_NULL`, `LIKE` family, `CONCAT`, `CASE*`, `EXISTS`); everything else (function-call-shaped operators
like `abs({0})`, `substring({0},{1})`, all of `MathOps`, all of the plain `DateTimeOps` field extractors)
implicitly gets precedence `-1` (`HIGHEST`, i.e. "never needs parens") via the single-arg `add(op, pattern)`
overload (`protected final void add(Operator op, String pattern) { ...; if (!precedence.containsKey(op))
precedence.put(op, -1); }`, lines 292-297). `JavaTemplates` (`types/JavaTemplates.java`) is a sibling
`Templates` subclass used by `querydsl-collections` to render expressions as literal Java code for
in-memory evaluation against POJOs (e.g. `Ops.LOWER -> "{0}.toLowerCase()"`, `Ops.BETWEEN -> "{1} <= {0} &&
{0} <= {2}"`) - a second, parallel serialization target sharing the same operator/precedence
infrastructure. Every SQL dialect (`OracleTemplates`, `MySQLTemplates`, ...) and `JPQLTemplates` subclass
`Templates` and override individual `add(...)` calls or `setPrecedence(...)` for dialect quirks - the
precedence *ladder* itself is inherited unchanged; only per-operator template strings and occasional
precedence reassignments vary per backend. Source: `types/Templates.java` (whole file, 344 lines);
`types/JavaTemplates.java` (whole file, 104 lines).

For the Rust port: this is a straightforward pretty-printer concern (a `HashMap<Operator, (Template, i32)>`
or a `match`-based precedence function plus a Pratt-style or recursive "wrap in parens if child precedence
> my precedence" printer) with no semantic surprises - the interesting part to preserve faithfully is that
the *default* for any operator not explicitly given a precedence is "never parenthesize" (function-call
syntax is inherently unambiguous), not "always parenthesize" or "inherit parent precedence".

---

## 5. Prioritized port list for Rust

**P0 - required for any usable query DSL, no shortcuts:**
- General/equality: `EQ`, `NE`, `IS_NULL`, `IS_NOT_NULL`, `IN`, `NOT_IN` (with the same literal-vs-expression
  dual overloads and the null-rejecting `eq`/`ne` behavior from 4.1/4.5).
- Boolean logic: `AND`, `OR`, `NOT`, plus the null-safe `and`/`or` combinators from 4.1 (essential for any
  `BooleanBuilder`-equivalent incremental predicate construction).
- Comparison: `GOE`, `GT`, `LOE`, `LT`, `BETWEEN` (with the one-sided-null-degrades-to-goe/loe behavior).
- Numeric core: `ADD`, `SUB`, `MULT`, `DIV`, `NEGATE`, `MOD`, plus `ABS`/`CEIL`/`FLOOR`/`ROUND` (the six
  math ops that got promoted to instance methods in Java, i.e. the ones users reach for constantly).
- String core: `CONCAT`, `LOWER`, `UPPER`, `TRIM`, `SUBSTR_1ARG`/`SUBSTR_2ARGS`, `STRING_LENGTH`,
  `STRING_IS_EMPTY`, `STARTS_WITH`, `ENDS_WITH`, `STRING_CONTAINS`, `LIKE` (+ `LIKE_ESCAPE`), `INDEX_OF`.
- Aggregate: `COUNT_AGG`, `COUNT_DISTINCT_AGG`, `MIN_AGG`, `MAX_AGG`, `AVG_AGG`, `SUM_AGG` (with the
  numeric-widening choice `sumLong`/`sumDouble`/`sumBigDecimal` forces onto callers - a Rust port can likely
  do this more elegantly with a single generic `sum<R>()` bound by a `From`/numeric-promotion trait).
- Case/coalesce: `CASE`/`CASE_WHEN`/`CASE_ELSE`, `COALESCE`, `NULLIF` - ubiquitous in real query code.
- Collection basics: `COL_IS_EMPTY`, `COL_SIZE`, and the Path-based `LISTVALUE`/`MAPVALUE`/`ARRAYVALUE`
  indexed-access mechanism (not `Ops` operators, but essential AST shape - see section 4).
- The `Expression`/`Operation`/`Path`/`Constant`/`Visitor` core interfaces and the `Operator` result-type
  contract (section 3.2) - get the AST shape right first; everything else is additive.

**P1 - important, expected by any serious user, but the DSL is usable without them on day one:**
- Date/time field extraction: `YEAR`, `MONTH`, `DAY_OF_MONTH`, `DAY_OF_WEEK`, `DAY_OF_YEAR`, `WEEK`, `HOUR`,
  `MINUTE`, `SECOND`, `MILLISECOND`, `YEAR_MONTH`, `YEAR_WEEK`, plus `CURRENT_DATE`/`CURRENT_TIME`/
  `CURRENT_TIMESTAMP`.
- Remaining string helpers: `EQ_IGNORE_CASE`, `*_IC` case-insensitive family, `CHAR_AT`, all of `StringOps`
  (`LEFT/RIGHT/LTRIM/RTRIM/LPAD/RPAD/LPAD2/RPAD2/LOCATE/LOCATE2`).
- `NOT_IN`'s quantified siblings: `eqAll/eqAny/gtAll/gtAny/goeAll/goeAny/ltAll/ltAny/loeAll/loeAny` and the
  `QuantOps.ANY/ALL` machinery behind them - needed once subqueries are supported at all.
- Remaining math functions: all 22 static-only `MathOps` entries (`ACOS...TANH`, `POWER`, `LOG`, `SIGN`,
  `DEG`/`RAD`, `RANDOM`/`RANDOM2`) - straightforward once the operator/operation plumbing exists, low risk,
  but genuinely optional for a first usable release.
- `NUMCAST`/`STRING_CAST` (type-casting sugar) and `ALIAS` (result-column naming) - needed once the port
  supports projections/tuples, which is presumably a different report's territory but the operators
  themselves belong in this inventory.
- `CASE_EQ`/`CASE_EQ_WHEN`/`CASE_EQ_ELSE` (the switch-shaped case sugar) - nice ergonomics, mechanically
  derivable from the general `CASE` support once that exists, so lower risk than it looks.
- Map operators: `CONTAINS_KEY`, `CONTAINS_VALUE`, `MAP_SIZE`, `MAP_IS_EMPTY`; Array: `ARRAY_SIZE`.
- `EXISTS` (subquery existence) - depends on whatever subquery design the Rust port adopts; flag as
  coupled to that decision rather than standalone.

**P2 - nice-to-have, defer without regret, or reconsider the design entirely rather than port as-is:**
- The full SQL-module-only date-arithmetic surface: `ADD_YEARS...ADD_SECONDS`, `DIFF_YEARS...DIFF_SECONDS`,
  `TRUNC_YEAR...TRUNC_SECOND`, `DATE`. In Java these are SQL-dialect-dependent (some databases don't support
  a given unit) and are only reachable via `SQLExpressions`, never via core - a Rust port might reasonably
  choose a cleaner unified `date_add(unit, expr, amount)` design from day one instead of porting the
  per-unit method explosion verbatim.
- `AVG_IN_COL`/`MAX_IN_COL`/`MIN_IN_COL` (JPA-only correlated-collection aggregates) - narrow HQL-specific
  feature, only relevant if/when a JPA-equivalent backend is planned.
- `BOOLEAN_ALL`/`BOOLEAN_ANY` (SQL-only boolean aggregates) - narrow, Postgres/MySQL-flavored, low usage.
- `XOR`/`XNOR`/`MATCHES_IC`/`SYSDATE`/`WRAPPED`/`ORDER`/`Ops.compareOps`/`Ops.equalsOps`/`Ops.notEqualsOps` -
  confirmed dead or effectively unreachable in the Java original (section 1, "no DSL exposure" /
  "zero usages" findings). Do not port these as user-facing API; if the AST needs an internal
  forced-parenthesization node (`WRAPPED`'s role) that's an implementation detail, not a public operator.
  `SYSDATE` in particular looks like it should be deleted from `Ops` upstream rather than preserved.
- The ~10x duplicated `as(Path<T>)`/`as(String)` override-per-subclass ceremony (section 3.3, last bullet)
  - a pure Java-generics workaround; a Rust trait with a `Self`-returning method makes all of it
    unnecessary, so there is nothing to "port" here beyond the single trait method.
- `LiteralExpression`'s split-hierarchy wrinkle (NumberExpression bypassing it) - don't reproduce the
  duplication; design the Rust trait hierarchy so numeric types share the cast/stringify surface with
  everything else from the start (section 3.3, `NumberExpression` re-implements bullet).

---

## Unresolved questions

- Whether the Rust port intends to support a JPA/HQL-equivalent backend at all; several P1/P2 items
  (`QuantOps.AVG_IN_COL` family, `EXISTS` semantics inside correlated subqueries) are meaningfully
  different in shape between SQL and HQL and the prioritization above assumes a SQL-first port.
- Whether `Coalesce`'s two-phase mutable-builder-then-freeze design (section 4.3) should be preserved as-is
  in Rust (a builder type) or collapsed into a single `coalesce(exprs: &[Expr]) -> Expr` free function now
  that Rust doesn't need the per-return-type `asX()` dispatch trick Java uses to pick the wrapper class.
- Whether the numeric `sum()` API should keep Java's caller-chosen-widening shape (`sumLong`/`sumDouble`/
  `sumBigDecimal`/`sumBigInteger`) or move the widening decision into the type system via a generic bound -
  flagged in the P0 list but the actual resolution affects the `NumberExpr` trait's shape significantly and
  probably deserves its own design note before implementation.

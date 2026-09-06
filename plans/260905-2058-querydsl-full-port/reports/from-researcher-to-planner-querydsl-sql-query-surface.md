# QueryDSL SQL Query Surface - Feature Inventory for Rust Port

Scope: querydsl-sql query construction surface (SELECT/DML/window functions/metadata/flag mechanism).
Source root: `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql` and `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core`.
All paths below are repo-relative to `querydsl/`.

## 1. SELECT surface

### 1.1 Entry points (factory-level)

Defined in `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/AbstractSQLQueryFactory.java` (abstract, implemented by `SQLQueryFactory.java`) and mirrored as static helpers in `SQLExpressions.java`.

| Method | Source | Notes |
|---|---|---|
| `select(Expression<T>)` / `select(Expression<?>...)` | `SQLQueryFactory.java:74-81` | second form returns `Tuple` |
| `selectDistinct(Expression<T>)` / `selectDistinct(Expression<?>...)` | `SQLQueryFactory.java:83-91` | `= select(...).distinct()` |
| `selectZero()` / `selectOne()` | `SQLQueryFactory.java:93-101` | `select(0)` / `select(1)` |
| `selectFrom(RelationalPath<T>)` | `SQLQueryFactory.java:103-106` | `select(expr).from(expr)` |
| `query()` | `SQLQueryFactory.java:68-71` | bare `SQLQuery<Void>` |

```java
// SQLQueryFactory.java:104-106
public <T> SQLQuery<T> selectFrom(RelationalPath<T> expr) {
  return select(expr).from(expr);
}
```

The same shapes exist as static, detached-query helpers on `SQLExpressions.java:155-224` (`SQLExpressions.select/selectDistinct/selectZero/selectOne/selectFrom/union/unionAll`), useful for building subqueries without a factory instance.

### 1.2 from() - multiple sources

`SQLCommonQuery.java:85` `from(RelationalPath<?>...)`; `ProjectableSQLQuery.java:162-184` adds single-arg, varargs-with-extra-args, and subquery-as-source overloads.

```java
// ProjectableSQLQuery.java:182-184
public Q from(SubQueryExpression<?> subQuery, Path<?> alias) {
  return queryMixin.from(ExpressionUtils.as((Expression) subQuery, alias));
}
```

### 1.3 Joins - all types

Declared in `SQLCommonQuery.java:96-337`, implemented in `ProjectableSQLQuery.java:186-309` (delegating to `QueryMixin`).
`JoinType` enum: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/JoinType.java` -> `DEFAULT` (cross), `INNERJOIN`, `JOIN`, `LEFTJOIN`, `RIGHTJOIN`, `FULLJOIN`.

Each join type has 5 overloads: `EntityPath`, `EntityPath+alias`, `RelationalFunctionCall+alias` (table-valued function joins), `SubQueryExpression+alias`, and `ForeignKey+RelationalPath` (FK-driven join, auto-generates `on()`).

```java
// ProjectableSQLQuery.java:207-209 - FK-driven join
public <E> Q fullJoin(ForeignKey<E> key, RelationalPath<E> entity) {
  return queryMixin.fullJoin(entity).on(key.on(entity));
}
```

```java
// SelectBase.java:399-403 (test) - compact FK join vs. verbose on()
query().from(employee).innerJoin(employee.superiorIdKey, employee2)
       .select(employee.id, employee2.id).fetch();
```

`straightJoin()` is NOT part of the common surface; it is a MySQL-dialect extension (`mysql/AbstractMySQLQuery.java:187-189`) implemented as a raw `addFlag(Position.AFTER_SELECT, "straight_join ")`, not a `JoinType`.

`on(Predicate...)` - `SQLCommonQuery.java:290`, chainable (multiple `.on()` calls AND together, see `SelectBase.java:1157-1159`).

### 1.4 where / groupBy / having / orderBy

- `where(Predicate...)`: `FilteredClause.java:34`.
- `groupBy(Expression<?>...)`, `having(Predicate...)`: `Query.java:37-45`.
- `orderBy(OrderSpecifier<?>...)`: `SimpleQuery.java:59`.
- Null handling: `OrderSpecifier.java:32-36` enum `NullHandling{Default,NullsFirst,NullsLast}`, plus fluent `.nullsFirst()/.nullsLast()` (`OrderSpecifier.java:95-106`).

```java
// WindowFunctionTest.java:28-31 - nulls-first ordering
SQLExpressions.sum(path).over().partitionBy(path2).orderBy(path.desc().nullsFirst());
// -> "sum(path) over (partition by path2 order by path desc nulls first)"
```

### 1.5 limit / offset / restrict / distinct / distinctOn

- `limit(long)`, `offset(long)`, `restrict(QueryModifiers)`: `SimpleQuery.java:35-51`. `QueryModifiers.java` is an immutable `(limit, offset)` pair with validation (limit > 0, offset >= 0) and a `subList()` helper for in-memory pagination.
- `distinct()`: `SimpleQuery.java:76`.
- `distinctOn(Expression<?>...)` is PostgreSQL-only: `postgresql/AbstractPostgreSQLQuery.java:93-98`, implemented as `addFlag(AFTER_SELECT, "distinct on({0}) ", ...)`.

```java
// PostgreSQLQueryTest.java:96-104
query.from(employee)
     .distinctOn(employee.datefield, employee.timefield)
     .orderBy(employee.datefield.asc(), employee.timefield.asc(), employee.salary.asc())
     .select(employee.id);
```

### 1.6 Locking: forUpdate / forShare / noWait / skipLocked

`AbstractSQLQuery.java:156-197`: `forUpdate()` and `forShare(boolean fallbackToForUpdate)` read dialect flags from `SQLTemplates` (`getForUpdateFlag/getForShareFlag`, `SQLTemplates.java:254-258,801-809`) and throw `QueryException` if `forShare()` is unsupported and no fallback requested.
`noWait()` and `of(RelationalPath...)` (FOR UPDATE/SHARE OF table) are PostgreSQL-only: `postgresql/AbstractPostgreSQLQuery.java:60-85`.

```java
// PostgreSQLQueryTest.java:85-93
query.from(survey).forUpdate().noWait();   // "... for update nowait"
query.from(survey).forUpdate().of(survey); // "... for update of SURVEY"
```

**`skipLocked()` / SKIP LOCKED does not exist anywhere in this codebase** (verified via full-repo grep). This is a gap versus modern Postgres/Oracle/MySQL 8+ support - flag as a P1 addition for the Rust port even though the Java source lacks it.

### 1.7 addFlag / addJoinFlag - the raw-SQL escape hatch

Covered in detail in section 6. Core API: `SQLCommonQuery.java:40-77`, `ProjectableSQLQuery.java:89-154`.

### 1.8 union / unionAll

Two shapes exist on `ProjectableSQLQuery.java:311-406`:
1. **As a `Union<RT>` result** (`Fetchable`, not attached to `from()`): `union(SubQueryExpression<RT>...)` / `union(List<...>)` / `unionAll(...)` -> returns `UnionImpl` (`UnionImpl.java`) supporting `fetch/fetchFirst/fetchOne/iterate/stream/fetchResults/fetchCount/groupBy/having/orderBy/as(...)`.
2. **As a subquery source**: `union(Path<?> alias, SubQueryExpression<RT>...)` / `unionAll(Path<?> alias, ...)` -> calls `from(UnionUtils.union(...))`, i.e. produces a derived table usable in a further `from()`.

Interface: `Union.java`. Static detached-query helpers: `SQLExpressions.union/unionAll` (`SQLExpressions.java:226-262`).

```java
// UnionBase.java:47-56 (test)
SubQueryExpression<Integer> sq1 = query().from(employee).select(employee.id.max().as("ID"));
SubQueryExpression<Integer> sq2 = query().from(employee).select(employee.id.min().as("ID"));
query().union(sq1, sq2).orderBy(employee.id.asc()).fetch();
```

```java
// UnionBase.java:32-44 (test) - union used inside an IN subquery
query().from(employee)
       .where(employee.id.in(query().union(query().select(Expressions.ONE), query().select(Expressions.TWO))))
       .select(Expressions.ONE).fetchFirst();
```

Note: INTERSECT/EXCEPT are not implemented (see PostgreSQLQueryTest.java:41-42 - "TODO INTERSECT"/"TODO EXCEPT" comment in the syntax doc test).

### 1.9 with / withRecursive (CTE)

`SQLCommonQuery.java:339-433`, implemented `ProjectableSQLQuery.java:419-456`. Three shapes per with/withRecursive: `(alias, SubQueryExpression)`, `(alias, Expression)`, and `(alias, Path...columns)` returning a `WithBuilder<Q>` whose `.as(Expression)` supplies the body (for explicit column lists). `withRecursive` additionally injects a `QueryFlag(WITH, "recursive")` marker (`SQLTemplates.RECURSIVE`).

```java
// SelectBase.java:2385-2395 (test)
query().with(employee2, query().from(employee).where(employee.firstname.eq("Jim")).select(Wildcard.all))
       .from(employee, employee2)
       .select(employee.id, employee2.id)
       .fetch();
```

```java
// SelectBase.java:2417-2424 (test) - explicit column list CTE
query().with(employee2, employee2.all())
       .as(query().from(employee).where(employee.firstname.eq("Jim")).select(Wildcard.all))
       .from(employee, employee2).select(employee.id, employee2.id).fetch();
```

`WithBuilder<R>`: `WithBuilder.java`.

### 1.10 Subqueries (in / exists / scalar / correlated)

Subqueries are just `SQLQuery` instances used as `Expression`/`SubQueryExpression`; no special subquery type exists beyond the base query class. `.exists()`/`.notExists()`/scalar coercion come from core `SubQueryExpression`/`FetchableSubQueryBase` (not re-derived here, out of this report's direct file list, but exercised below).

```java
// SelectBase.java:2373-2381 (test) - exists / not exists
SQLQuery<Integer> sq1 = query().from(employee).select(employee.id.max());
query().from(employee).where(sq1.exists()).fetchCount();
query().from(employee).where(sq1.exists().not()).fetchCount();
```

```java
// SelectBase.java:905-918 (test) - correlated subquery (references outer alias)
query().from(employee)
       .where(query().from(employee2).where(employee2.id.eq(employee.id))
                      .select(employee2.id, employee2.id).exists())
       .fetchCount();
```

```java
// SelectBase.java:1077 (test) - IN subquery / IN list
query().from(employee).where(employee.id.in(Arrays.asList(1, 2)));
```

Scalar subquery: any `select(singleExpr)` query used directly as an `Expression<T>` in another expression tree (e.g. `SQLExpressions.datediff(dp, Expressions.currentTimestamp(), timestamp)`, `SelectBase.java:721`).

### 1.11 Tuple selection

`select(Expression<?>...)` (2+ columns, no bean/factory expression) yields `Query<Tuple>`; `Tuple.get(index, type)` / `Tuple.get(Expression)` accessors (`core/Tuple.java`, not modified here). `SQLQuery<T>.select(Expression<?>...)` override: `SQLQuery.java:133-139`.

### 1.12 transform()

`FetchableQuery.java:52`: `<S> S transform(ResultTransformer<S> transformer)`. Primary consumer is `com.querydsl.core.group.GroupBy` (result-set grouping into nested maps/lists), used as:

```java
// SelectBase.java:952-961 (test)
SQLQuery<?> qry = query().from(employee).innerJoin(employee._superiorIdKey, employee2);
var subordinates = Projections.tuple(employee2.id, employee2.firstname, employee2.lastname);
Map<Integer, Group> results = qry.transform(
    GroupBy.groupBy(employee.id).as(employee.firstname, employee.lastname,
                                     GroupBy.map(employee2.id, subordinates)));
```

### 1.13 fetch/fetchOne/fetchFirst/fetchCount/fetchResults/iterate/stream

All defined in `Fetchable.java` (interface) and implemented in `AbstractSQLQuery.java`:
- `fetch()` (`AbstractSQLQuery.java:423-500`) - materializes full `List<T>`, handles `FactoryExpression`, `Wildcard.all` (Object[] rows), and scalar projections.
- `fetchOne()` (`ProjectableSQLQuery.java:408-416`) - auto-applies `limit(2)` then throws `NonUniqueResultException` if >1 row.
- `fetchFirst()` - default via `Fetchable`, effectively `limit(1)` + first-or-null.
- `fetchCount()` (`AbstractSQLQuery.java:137-146`, overridden per dialect via `unsafeCount()` using `select count(*)`).
- `fetchResults()` (`AbstractSQLQuery.java:502-547`) - returns `QueryResults<T>` (rows + total count); optimizes via a `count(*) over()` window flag (`rowCountFlag`, `AbstractSQLQuery.java:62-63`) when `SQLTemplates.isCountViaAnalytics()` is true and there's no `GROUP BY`, else falls back to a separate count query.
- `iterate()` (`AbstractSQLQuery.java:340-421`) - streaming `CloseableIterator<T>` over a live `ResultSet`.
- `stream()` - default method on `Fetchable.java:67-72`, wraps `iterate()` in a `Spliterator`-backed `Stream` that closes the iterator on stream close.

## 2. DML surface

### 2.1 Common shape

All DML clauses extend `AbstractSQLClause<C>` (`dml/AbstractSQLClause.java`) implementing core `DMLClause<C>{ execute() }`. Batches, listeners, statement lifecycle (prepare/set-params/execute/translate-exceptions) are all handled here; dialect subclasses only override rendering via `Configuration`/`SQLTemplates`.

### 2.2 INSERT

`dml/SQLInsertClause.java` (concrete) extends `dml/AbstractSQLInsertClause.java` implementing core `InsertClause<C> extends StoreClause<C>` (`core/dml/InsertClause.java`, `StoreClause.java`).

| Capability | Method | Source |
|---|---|---|
| columns | `columns(Path<?>...)` | `AbstractSQLInsertClause.java:172-176` |
| values | `values(Object...)` | `AbstractSQLInsertClause.java:527-539` |
| per-column set | `set(Path<T>, T)` / `set(Path<T>, Expression<? extends T>)` / `setNull(Path<T>)` | `AbstractSQLInsertClause.java:500-525` |
| insert-select | `select(SubQueryExpression<?>)` or constructor `(conn, config, entity, SQLQuery<?> subQuery)` | `AbstractSQLInsertClause.java:491-498`, `.java:83-113` |
| batch | `addBatch()` | `AbstractSQLInsertClause.java:144-154`; bulk-optimized via `setBatchToBulk(true)` (`.java:160-162`) which serializes N rows as one multi-row INSERT (`SQLInsertBatch.java`) when `SQLTemplates.isBatchToBulkSupported()` |
| bean population | `populate(Object)` / `populate(T, Mapper<T>)` | `AbstractSQLInsertClause.java:559-577` |
| execute | `execute()` -> row count | `AbstractSQLInsertClause.java:423-467` |
| execute + keys | `executeWithKey(Path<T>)`, `executeWithKey(Class<T>)`, `executeWithKeys(Path<T>)`/`(Class<T>)`, `executeWithKeys()` -> raw `ResultSet` | `AbstractSQLInsertClause.java:178-421` |

```java
// InsertBase.java:141-149 (test) - batch insert
var insert = insert(survey).set(survey.id, 5).set(survey.name, "55").addBatch();
insert.set(survey.id, 6).set(survey.name, "66").addBatch();
insert.execute(); // == 2
```

```java
// InsertBase.java:254-256 (test) - columns + values
insert(survey).columns(survey.id, survey.name).values(3, "Hello").execute();
```

`executeWithKey(s)` implements RETURNING-like key retrieval: it calls JDBC `Statement.RETURN_GENERATED_KEYS` (or explicit PK column names when `entity.getPrimaryKey()` is known, `AbstractSQLInsertClause.java:340-352`) and reads back `stmt.getGeneratedKeys()`. This is the closest Querydsl gets to a native `RETURNING` clause - it is JDBC-generated-keys based, not `INSERT ... RETURNING` SQL syntax.

### 2.3 UPDATE

`dml/SQLUpdateClause.java` (thin, not separately read - trivial constructor wrapper) extends `dml/AbstractSQLUpdateClause.java`, implementing core `UpdateClause<C> extends StoreClause<C>, FilteredClause<C>` (`core/dml/UpdateClause.java`).

| Capability | Method | Source |
|---|---|---|
| set | `set(Path<T>, T)`, `set(Path<T>, Expression<? extends T>)`, `set(List<Path>, List<Object>)` | `AbstractSQLUpdateClause.java:254-294` |
| setNull | `setNull(Path<T>)` | `.java:276-280` |
| where | `where(Predicate...)` | `.java:296-307` |
| limit | `limit(long)` (dialect-dependent support) | `.java:309-312` |
| batch | `addBatch()` | `.java:111-117` |
| populate | `populate(Object)` / `populate(T, Mapper<T>)` - **skips primary-key columns** | `.java:328-353` |
| execute | `execute()` -> row count | `.java:198-235` |

```java
// UpdateBase.java-style usage (pattern seen across dml tests)
update(survey).set(survey.name, "AAA").where(survey.id.eq(1)).execute();
```

### 2.4 DELETE

`dml/SQLDeleteClause.java` (thin) extends `dml/AbstractSQLDeleteClause.java`, implementing core `DeleteClause<C> extends DMLClause<C>, FilteredClause<C>` (`core/dml/DeleteClause.java`).
Notably installs a `ValidatingVisitor` (`AbstractSQLDeleteClause.java:53-59`) that throws if the where-clause references a second table ("A delete operation can only reference a single table... Consider `DELETE ... WHERE EXISTS (subquery)`").
Supports `where(...)`, `limit(...)`, `addBatch()`, `execute()`.

```java
// InsertBase.java:63-65 (test, setup/teardown pattern)
delete(survey).execute();
```

### 2.5 MERGE

Two independent APIs exist:

**(a) Classic key-based `SQLMergeClause`** (`dml/SQLMergeClause.java`) implements core `StoreClause<SQLMergeClause>`:
- `keys(Path<?>...)` defines the match columns (falls back to `entity.getPrimaryKey()` if omitted, `SQLMergeClause.java:134-142,588-591`).
- `columns(...)`, `values(...)`, `set(...)`, `setNull(...)`, `select(SubQueryExpression)` - same shape as insert.
- `addBatch()` - only permitted when `SQLTemplates.isNativeMerge()` (else throws `IllegalStateException`).
- `execute()` dispatches to `executeNativeMerge()` (real `MERGE INTO ... USING ... WHEN MATCHED/NOT MATCHED`) when the dialect supports native merge, otherwise `executeCompositeMerge()` which does `hasRow()` (a `SELECT 1 ... WHERE <key predicates>`) then emulates via a plain `SQLUpdateClause` or `SQLInsertClause` (`SQLMergeClause.java:326-333,358-398`).
- `executeWithKey(s)` mirrors insert's generated-key retrieval.

**(b) Fluent `USING`/`WHEN MATCHED` builder `SQLMergeUsingClause` + `SQLMergeUsingCase`** (`dml/SQLMergeUsingClause.java`, `dml/SQLMergeUsingCase.java`), reached via `mergeClause.using(SimpleExpression<?>)`:
- `.on(Predicate...)` - merge join condition.
- `.whenMatched()` / `.whenNotMatched()` -> `SQLMergeUsingCase` with `.and(Predicate)` (extra condition), then exactly one of `.thenInsert(paths, values)`, `.thenUpdate(paths, values)`, `.thenDelete()` (enum `MergeOperation{UPDATE,INSERT,DELETE}`) which calls back `parentClause.addWhen(this)`.
- Always native-merge only (`execute()` -> `executeNativeMerge()`, no composite fallback).

```java
// MergeUsingBase.java:53-71 (test) - full USING/WHEN MATCHED/NOT MATCHED chain
merge(survey)
    .using(query().from(survey2).select(survey2.id.add(40).as("ID"), survey2.name).as(usingSubqueryAlias))
    .on(survey.id.eq(usingSubqueryAlias.id))
    .whenNotMatched()
      .thenInsert(Arrays.asList(survey.id, survey.name), Arrays.asList(usingSubqueryAlias.id, usingSubqueryAlias.name))
    .whenMatched()
      .and(survey.id.goe(10))
      .thenDelete()
    .whenMatched()
      .thenUpdate(Collections.singletonList(survey.name), Collections.singletonList(usingSubqueryAlias.name))
    .execute();
```

### 2.6 Batch semantics (all DML clauses)

- Each `addBatch()` snapshots the current column/value/where state into a `SQLInsertBatch` / `SQLUpdateBatch` / bare `QueryMetadata` (delete) / `SQLMergeBatch`, then resets builder state for the next row.
- `execute()` groups batches by identical rendered SQL string into `PreparedStatement`s keyed by SQL text (`Map<String, PreparedStatement>`), calling `addBatch()`/`executeBatch()` on the JDBC statement unless `configuration.getUseLiterals()` is true (literal mode disables real JDBC batching and just re-executes each statement, `AbstractSQLClause.java:188-201`).
- `executeBatch(Collection<PreparedStatement>)` sums affected-row counts across all distinct statements (`AbstractSQLClause.java:203-209`), respecting `SQLTemplates.isBatchCountViaGetUpdateCount()` for drivers that don't return per-row batch counts reliably.
- Insert-only: `setBatchToBulk(true)` collapses all batched rows into a single multi-row `INSERT ... VALUES (...), (...), ...` when the dialect supports it (`isBatchToBulkSupported()`), instead of N separate statements.

## 3. Window functions

### 3.1 API shape

Builder chain: `SQLExpressions.<fn>(...)` returns `WindowOver<T>` (`WindowOver.java`) -> `.over()` returns `WindowFunction<T>` (`WindowFunction.java`) -> `.partitionBy(...)`, `.orderBy(...)`, `.rows()`/`.range()` -> `WindowRows<T>` (`WindowRows.java`) -> `.unboundedPreceding()`/`.currentRow()`/`.preceding(n)` (terminal) or `.between()` -> `Between` -> `.unboundedPreceding()/.currentRow()/.preceding(n)/.following(n)` -> `BetweenAnd` -> `.unboundedFollowing()/.currentRow()/.preceding(n)/.following(n)` (terminal, returns back to `WindowFunction<T>`).

Alternative entry: `WindowOver<T>.keepFirst()`/`.keepLast()` -> `WindowFirstLast<T>` (`WindowFirstLast.java`, Oracle `KEEP (DENSE_RANK FIRST/LAST ORDER BY ...)`) -> `.orderBy(...)` -> `.over()` -> back to `WindowFunction<T>`.

Aggregate-as-hypothetical-set entry: some functions (`rank`, `denseRank`, `percentRank`, `cumeDist`) have overloads returning `WithinGroup<T>` (`WithinGroup.java`) instead of `WindowOver<T>`, exposing `.withinGroup()` -> `OrderBy` -> `.orderBy(...)` (`WITHIN GROUP (ORDER BY ...)` syntax, used by `listagg`, `percentileCont/Disc`, ordered-set aggregates).

```java
// WindowFunctionTest.java:16-22
Expression<?> wf = SQLExpressions.sum(path).over().partitionBy(path2).orderBy(path);
// -> "sum(path) over (partition by path2 order by path asc)"
```

```java
// WindowFunctionTest.java:90-106 - rows/range + between + unbounded/current/preceding/following
var wf = SQLExpressions.sum(path).over().orderBy(path);
wf.rows().between().currentRow().unboundedFollowing();
// "... rows between current row and unbounded following"
wf.rows().between().preceding(intPath).following(intPath);
wf.rows().between().preceding(1).following(3);
wf.rows().unboundedPreceding();
wf.rows().currentRow();
wf.rows().preceding(3);
```

```java
// WindowFunctionTest.java:139-149 - Oracle KEEP (DENSE_RANK FIRST/LAST)
SQLExpressions.min(path).keepFirst().orderBy(path2);
// "min(path) keep (dense_rank first order by path2 asc)"
SQLExpressions.min(path).keepFirst().orderBy(path2).over().partitionBy(path3);
// "min(path) keep (dense_rank first order by path2 asc) over (partition by path3)"
```

Note: `range()` mirrors `rows()` exactly (same `WindowRows` class, prefix `" range"` vs `" rows"`, `WindowFunction.java:193-203`). No dedicated `WindowRowsBetween.java` file exists - "between" logic lives as inner classes `Between`/`BetweenAnd` inside `WindowRows.java`.

### 3.2 Window-capable functions catalog (`SQLExpressions.java`)

| Function | Signature | Returns | SQLOps / Ops backing |
|---|---|---|---|
| `sum` | `sum(Expression<T extends Number>)` | `WindowOver<T>` | `Ops.AggOps.SUM_AGG` |
| `count` | `count()`, `count(Expression<?>)` | `WindowOver<Long>` | `AggOps.COUNT_ALL_AGG`/`COUNT_AGG` |
| `countDistinct` | `countDistinct(Expression<?>)` | `WindowOver<Long>` | `AggOps.COUNT_DISTINCT_AGG` |
| `avg` | `avg(Expression<T extends Number>)` | `WindowOver<T>` | `AggOps.AVG_AGG` |
| `min` / `max` | `min/max(Expression<T extends Comparable>)` | `WindowOver<T>` | `AggOps.MIN_AGG`/`MAX_AGG` |
| `lead` / `lag` | `lead/lag(Expression<T>)` | `WindowOver<T>` | `SQLOps.LEAD`/`LAG` |
| `nthValue` | `nthValue(Expression<T>, Number\|Expression<Number>)` | `WindowOver<T>` | `SQLOps.NTHVALUE` |
| `ntile` | `ntile(T num)` | `WindowOver<T>` | `SQLOps.NTILE` |
| `rowNumber` | `rowNumber()` | `WindowOver<Long>` | `SQLOps.ROWNUMBER` |
| `rank` | `rank()` (analytic) / `rank(Object...\|Expression<?>...)` (hypothetical-set aggregate) | `WindowOver<Long>` / `WithinGroup<Long>` | `SQLOps.RANK`/`RANK2` |
| `denseRank` | `denseRank()` / `denseRank(Object...\|Expression<?>...)` | `WindowOver<Long>` / `WithinGroup<Long>` | `SQLOps.DENSERANK`/`DENSERANK2` |
| `percentRank` | `percentRank()` / `percentRank(Object...\|Expression<?>...)` | `WindowOver<Double>` / `WithinGroup<Double>` | `SQLOps.PERCENTRANK`/`PERCENTRANK2` |
| `cumeDist` | `cumeDist()` / `cumeDist(Object...\|Expression<?>...)` | `WindowOver<Double>` / `WithinGroup<Double>` | `SQLOps.CUMEDIST`/`CUMEDIST2` |
| `percentileCont` / `percentileDisc` | `(T arg)` / `(Expression<T> arg)`, validated 0..1 | `WithinGroup<T>` | `SQLOps.PERCENTILECONT`/`PERCENTILEDISC` |
| `listagg` | `listagg(Expression<?>, String delimiter)` | `WithinGroup<String>` | `SQLOps.LISTAGG` |
| `ratioToReport` | `ratioToReport(Expression<T>)` | `WindowOver<T>` | `SQLOps.RATIOTOREPORT` |
| `firstValue` / `lastValue` | `(Expression<T>)` | `WindowOver<T>` | `SQLOps.FIRSTVALUE`/`LASTVALUE` |
| `stddev`, `stddevDistinct`, `stddevPop`, `stddevSamp` | `(Expression<T extends Number>)` | `WindowOver<T>` | `SQLOps.STDDEV*` |
| `variance`, `varPop`, `varSamp` | `(Expression<T extends Number>)` | `WindowOver<T>` | `SQLOps.VARIANCE`/`VARPOP`/`VARSAMP` |
| `corr`, `covarPop`, `covarSamp` | `(Expression, Expression)` | `WindowOver<Double>` | `SQLOps.CORR`/`COVARPOP`/`COVARSAMP` |
| `regrSlope/Intercept/Count/R2/Avgx/Avgy/Sxx/Syy/Sxy` | `(Expression, Expression)` | `WindowOver<Double>` | `SQLOps.REGR_*` (9 regression aggregates) |

Plus `WindowFirstLast` (Oracle `KEEP (DENSE_RANK FIRST/LAST ...)`) reached via `WindowOver.keepFirst()/keepLast()`, and Teradata's dialect-only `qualify(Predicate)` filter on window results (`sql/teradata/TeradataQuery.java:71-73`, backed by `SQLOps.QUALIFY`) - not part of the common `SQLCommonQuery` surface.

## 4. SQLExpressions catalog (non-window static helpers)

All in `sql/SQLExpressions.java` (1389 lines total).

| Category | Method(s) | Notes |
|---|---|---|
| Constants | `all` (`Wildcard.all`), `countAll` (`Wildcard.count`) | |
| Assignment | `set(Path<T>, Expression<? extends T>)`, `set(Path<T>, T)` | used inside upsert-style expressions |
| Query builders | `select`, `selectDistinct`, `selectZero`, `selectOne`, `selectFrom`, `union`, `unionAll` | detached-query equivalents of factory methods (sec. 1.1/1.8) |
| Boolean aggregates | `any(BooleanExpression)`, `all(BooleanExpression)` | `Ops.AggOps.BOOLEAN_ANY/ALL` |
| Generic function calls | `relationalFunctionCall(Class, String, Object...)` (table-valued, for `join(...)`), `function(Class, String, Object...)`, `stringFunction(...)`, `numberFunction(...)` | support dot-qualified names for cross-db/cross-schema calls, e.g. `function("other_db.dbo.my_function", arg)` |
| Sequences | `nextval(String)`, `nextval(Class<T>, String)` | `SQLOps.NEXTVAL`; **no `currval` exists in this codebase** (verified via grep) |
| Date conversion | `date(DateTimeExpression)`, `date(Class, DateTimeExpression)` | timestamp -> date |
| Date arithmetic (unit-based) | `dateadd(DatePart, DateTimeExpression\|DateExpression, int)` | `DatePart` enum: `year,month,week,day,hour,minute,second,millisecond` (`DatePart.java`) |
| Date diff | `datediff(DatePart, start, end)` - 6 overloads mixing `DateExpression`/`DateTimeExpression`/raw `D` on either side | returns `NumberExpression<Integer>` |
| Date truncation | `datetrunc(DatePart, DateExpression\|DateTimeExpression)` | |
| Date arithmetic (named units) | `addYears/addMonths/addWeeks/addDays/addHours/addMinutes/addSeconds` - each with `DateTimeExpression` and `DateExpression` overloads (`addHours/Minutes/Seconds` only on `DateTimeExpression`) | maps to `Ops.DateTimeOps.ADD_*` |
| String | `left(Expression<String>, int\|Expression<Integer>)`, `right(...)` | `Ops.StringOps.LEFT/RIGHT` |
| Aggregation | `groupConcat(Expression<String>)`, `groupConcat(Expression<String>, String separator)` | `SQLOps.GROUP_CONCAT`/`GROUP_CONCAT2` |
| Window functions | see section 3.2 | |

`regexp` matching is **not** a `SQLExpressions` static helper; it is exposed through the generic core DSL `.matches(String)` predicate (`Ops.MATCHES`), rendered per-dialect in `SQLTemplates.java:354` (`"{0} regexp {1}"`), `OracleTemplates.java:100` (`regexp_like`), `TeradataTemplates.java:79` (`regexp_instr`).

## 5. Metadata model

### 5.1 RelationalPath / RelationalPathBase

`RelationalPath<T>` (`sql/RelationalPath.java`) extends `EntityPath<T>` + `ProjectionRole<T>`:
- `getSchemaAndTable()` / `getSchemaName()` / `getTableName()` -> naming.
- `getColumns()` -> `List<Path<?>>`.
- `getPrimaryKey()` -> `PrimaryKey<T>` (nullable).
- `getForeignKeys()` / `getInverseForeignKeys()` -> `Collection<ForeignKey<?>>`.
- `getMetadata(Path<?> column)` -> `ColumnMetadata` (nullable).

`RelationalPathBase<T>` (`sql/RelationalPathBase.java`, 270 lines) is the base class generated Q-types extend. Constructors take `(type, variable|PathMetadata, schema, table)`, producing an immutable `SchemaAndTable`. Protected builder methods used by codegen/hand-written domain classes:
- `createPrimaryKey(Path<?>...)` -> `PrimaryKey<T>`.
- `createForeignKey(Path<?> local, String foreign)` / `createForeignKey(List<Path<?>>, List<String>)` -> `ForeignKey<F>`, registered into the path's `foreignKeys` list.
- `createInvForeignKey(...)` -> registered into `inverseForeignKeys` (the reverse direction, e.g. "employees referencing this department").
- `addMetadata(P path, ColumnMetadata metadata)` -> attaches column metadata (name override, nullability, size, JDBC type) keyed by `Path`.
- `count()`/`countDistinct()` -> memoized `NumberExpression<Long>` shortcuts for `count(*)` on this table.

### 5.2 ForeignKey / PrimaryKey

`sql/ForeignKey.java` and `sql/PrimaryKey.java` are both `@Immutable`, `Serializable`, `ProjectionRole<Tuple>`.
- `PrimaryKey<E>(RelationalPath<?> entity, Path<?>... localColumns)` - `getLocalColumns()`, `.in(CollectionExpression<?,Tuple>)` for tuple-IN predicates, `getProjection()` returns a `Tuple`-typed list expression of the PK columns.
- `ForeignKey<E>(RelationalPath<?> entity, List<Path<?>> localColumns, List<String> foreignColumns)` (or single-column ctor). Key method:

```java
// ForeignKey.java:77-86
public Predicate on(RelationalPath<E> entity) {
  var builder = new BooleanBuilder();
  for (int i = 0; i < localColumns.size(); i++) {
    var local = (Expression<Object>) localColumns.get(i);
    Expression<?> foreign = ExpressionUtils.path(local.getType(), entity, foreignColumns.get(i));
    builder.and(ExpressionUtils.eq(local, foreign));
  }
  return builder.getValue();
}
```

This is what powers both the compact `join(fk, entity)` overloads (section 1.3) and manual `on(fk.on(other))`:

```java
// SelectBase.java:1193-1194 (test) - manual on(fk.on(...))
query().from(employee).leftJoin(employee2).on(employee.superiorIdKey.on(employee2));
```

`ForeignKey` also supports composite multi-column keys (`localColumns`/`foreignColumns` are lists, matched positionally) and `.in(SubQueryExpression<Tuple>)` for row-value `IN` predicates against composite keys.

### 5.3 Schema + table naming

`sql/SchemaAndTable.java` - a tiny immutable value object `(schema, table)` with structural `equals`/`hashCode`. `RelationalPathBase` always constructs one at instantiation time; there is no separate "catalog" concept (3-part naming is dialect-rendering concern, e.g. `SQLTemplates`/`Configuration` column/table overrides), only schema+table.

## 6. QueryMetadata / QueryFlag mechanism (raw SQL escape hatch)

### 6.1 QueryMetadata

`core/QueryMetadata.java` is the mutable, `Serializable` bag holding **everything** about a query: projection, joins (`List<JoinExpression>`), where, groupBy, having, orderBy, `QueryModifiers` (limit/offset), params, distinct/unique flags, and the flag set. `DefaultQueryMetadata` is the concrete implementation (not separately inspected here beyond confirming it backs both `SQLQuery` and every DML clause's internal `metadata` field). Every fluent query method (`from`, `join`, `where`, ...) is a thin wrapper (via `QueryMixin`) that mutates a shared `QueryMetadata` instance - this is why `.clone()` exists on `QueryMetadata` (deep-ish copy for query reuse/subqueries).

### 6.2 QueryFlag - positional raw SQL injection

`core/QueryFlag.java`: an immutable `(Position, Expression<?> flag)` pair. `Position` enum (`QueryFlag.java:32-75`) enumerates every splice point in a serialized query:

```
WITH, START, START_OVERRIDE, AFTER_SELECT, AFTER_PROJECTION,
BEFORE_FILTERS, AFTER_FILTERS, BEFORE_GROUP_BY, AFTER_GROUP_BY,
BEFORE_HAVING, AFTER_HAVING, BEFORE_ORDER, AFTER_ORDER, END
```

`SQLCommonQuery.addFlag(Position, Expression|String)` / `addFlag(Position, String prefix, Expression)` (`SQLCommonQuery.java:33-59`, impl `ProjectableSQLQuery.java:108-154`) is the universal hook: the `SQLSerializer` inserts the flag's rendered SQL verbatim at the matching position during query serialization. This is how every dialect-specific keyword in this codebase is implemented without touching the core serializer - `forUpdate`, `forShare`, `distinctOn`, MySQL's `straight_join`/`sql_calc_found_rows`/`with rollup`/etc. (`mysql/AbstractMySQLQuery.java`), and `with`/`withRecursive` CTEs (which are literally `QueryFlag(Position.WITH, ...)` entries, `ProjectableSQLQuery.java:419-448`) are all just pre-packaged `addFlag` calls.

```java
// AbstractMySQLQuery.java:187-189 - dialect extension built purely on addFlag
public C straightJoin() {
  return addFlag(Position.AFTER_SELECT, STRAIGHT_JOIN);
}
```

### 6.3 JoinFlag - per-join raw SQL injection

`core/JoinFlag.java`: same idea scoped to a single join clause. `Position` enum: `START, OVERRIDE, BEFORE_TARGET, BEFORE_CONDITION, END`. `SQLCommonQuery.addJoinFlag(String)` (defaults to `BEFORE_TARGET`) / `addJoinFlag(String, JoinFlag.Position)` (`SQLCommonQuery.java:61-77`, impl `ProjectableSQLQuery.java:89-106`) attaches to **the most recently added join**. Used for MySQL index hints:

```java
// AbstractMySQLQuery.java:199-201
public C forceIndex(String... indexes) {
  return addJoinFlag(" force index (" + String.join(", ", indexes) + ")", JoinFlag.Position.END);
}
```

### 6.4 Design implication for the Rust port

The flag mechanism is the single generalization point that lets ~90% of "dialect-specific SQL feature" code (locking clauses, optimizer hints, MySQL SELECT modifiers, PostgreSQL `DISTINCT ON`, CTEs) be expressed as ordinary library code on top of a small, fixed set of splice points in the serializer, rather than growing the core query builder's API surface per dialect. A Rust port should treat `(Position, RenderedFragment)` injection as a first-class serializer concept from day one, not bolt it on later.

## 7. Prioritized port list for Rust

### P0 - core, everything else depends on this

- `QueryMetadata` equivalent: mutable struct holding projection/joins/where/group-by/having/order-by/modifiers/distinct/params/flags, with clone semantics for subquery reuse.
- `QueryFlag`/`JoinFlag` positional injection mechanism (section 6) - build the serializer around named splice points from the start.
- `SELECT` core: `select`/`selectDistinct`/`selectOne`/`selectZero`/`selectFrom`, `from` (multi-source), `where`, `groupBy`, `having`, `orderBy` with `NullHandling`, `limit`/`offset`/`restrict`, `distinct`.
- All 6 join types (`DEFAULT`/cross, `INNERJOIN`, `JOIN`, `LEFTJOIN`, `RIGHTJOIN`, `FULLJOIN`) + `on()`, including FK-driven join sugar (`ForeignKey::on`).
- `RelationalPath`/`RelationalPathBase`, `PrimaryKey`, `ForeignKey`, `SchemaAndTable` metadata model, including composite (multi-column) keys.
- `fetch/fetchOne/fetchFirst/fetchCount/iterate/stream` result materialization (JDBC-analog: whatever DB driver abstraction Rust uses).
- Basic INSERT (`columns/values/set/setNull/populate/execute/executeWithKey(s)`), UPDATE (`set/setNull/where/limit/execute`), DELETE (`where/limit/execute`), including single-table validation on delete.
- Subqueries as expressions: `exists()`/`notExists()`, `IN (subquery)`, scalar subquery usage anywhere an expression is expected, correlated subqueries (just outer-alias reuse, no special API).
- Tuple/multi-column projections (`select(a, b, ...)`).

### P1 - high value, needed for real-world parity

- Batch DML semantics: `addBatch()` across insert/update/delete/merge, statement de-duplication by rendered SQL, `executeBatch` row-count aggregation, insert-only bulk-collapse (`setBatchToBulk`).
- `union`/`unionAll` both as a fetchable result and as a derived-table subquery source (`from(union(...))`).
- `with`/`withRecursive` (CTEs), including the explicit-column-list `WithBuilder` form.
- Classic key-based `MERGE` (`keys/columns/values/set/select/addBatch`) with native-vs-composite-fallback execution strategy.
- Fluent `MERGE ... USING ... WHEN MATCHED/NOT MATCHED` builder (`SQLMergeUsingClause`/`SQLMergeUsingCase`) - increasingly the more common pattern (Postgres 15+, SQL Server, DB2).
- Full window function suite (section 3.2): ranking (`rowNumber/rank/denseRank/percentRank/cumeDist`), navigation (`lag/lead/firstValue/lastValue/nthValue`), distribution (`ntile/percentileCont/percentileDisc`), plus `rows()/range()/between()/unboundedPreceding/currentRow/unboundedFollowing`.
- `forUpdate()`/`forShare()` row locking with per-dialect capability flags (`isForShareSupported`) and fallback behavior.
- `fetchResults()` with the `count(*) over()` optimization path (`isCountViaAnalytics`) vs. separate-count fallback.
- `transform()` + a GroupBy-equivalent result transformer (nested map/list aggregation) - very commonly used for one-query parent/child hydration.
- `SQLExpressions` non-window catalog: `nextval`, `dateadd/datediff/datetrunc` + named `addYears/.../addSeconds`, `left/right`, `groupConcat`, generic `function/stringFunction/numberFunction` escape hatches, `relationalFunctionCall` for table-valued function joins.
- `addFlag`/`addJoinFlag` public API exposure (not just internal use) so downstream users can add their own raw-SQL extensions the way this codebase's dialect modules do.
- `noWait()` and **`skipLocked()`** row-lock modifiers - Postgres/Oracle/MySQL 8+ all support `SKIP LOCKED` even though upstream QueryDSL never added it; worth including in the Rust port as a deliberate improvement over the Java original.
- `distinctOn(...)` (Postgres-style `DISTINCT ON`).

### P2 - dialect-specific / lower-frequency, defer or make pluggable

- MySQL-only SELECT modifiers: `straightJoin`, `bigResult/bufferResult/cache/calcFoundRows/highPriority/noCache/smallResult`, `into/intoDumpfile/intoOutfile`, `lockInShareMode`, `forceIndex/ignoreIndex/useIndex`, `withRollup`.
- Oracle `KEEP (DENSE_RANK FIRST/LAST ORDER BY ...)` (`WindowFirstLast`) and `WITHIN GROUP (ORDER BY ...)` ordered-set aggregates (`WithinGroup`: `listagg`, `percentileCont/Disc`, hypothetical-set `rank/denseRank/percentRank/cumeDist`).
- Teradata `QUALIFY` predicate on window results.
- Regression/statistical window aggregates (`corr/covarPop/covarSamp/regrSlope/.../regrSxy`, `stddev*/variance/varPop/varSamp`) - real but niche; implement once core window plumbing exists since it's mechanically identical.
- `RelationalFunctionCall` (table-valued function join target) - needed for a handful of dialects (Postgres `generate_series`, SQL Server table-valued functions).
- `PostgreSQL`-specific `of(RelationalPath...)` (`FOR UPDATE OF table`).
- INTERSECT/EXCEPT - absent even in upstream Java (confirmed TODO comment in test), so this is a genuine gap to close only if product requirements demand it, not a parity requirement.
- `useLiterals` mode (inline literals instead of bind params) - a debugging/compatibility knob, not core semantics; also disables real JDBC batching when enabled, which is a footgun worth reconsidering in the Rust design rather than reproducing as-is.

## Unresolved questions

- Should the Rust port intentionally add `SKIP LOCKED` and `currval()` support (both absent upstream) as deliberate improvements, or strictly mirror the Java surface for a first pass and add these later as tracked follow-ups.
- Should INTERSECT/EXCEPT be added given upstream never implemented them (confirmed via the "TODO INTERSECT"/"TODO EXCEPT" comment in `PostgreSQLQueryTest.java`) - out of scope for a straight port but likely wanted for real-world SQL coverage.
- `executeWithKey(s)` is JDBC-generated-keys based, not `RETURNING`-syntax based; need a decision on whether the Rust port should model this as native `RETURNING` (better fit for Postgres/SQLite) with a JDBC-style fallback for dialects that lack it, rather than defaulting to the generated-keys strategy.

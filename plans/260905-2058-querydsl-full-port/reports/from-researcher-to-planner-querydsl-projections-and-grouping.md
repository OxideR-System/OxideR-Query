# QueryDSL Research: Result Projection and Grouping

Scope: how QueryDSL turns a query result row into a user type, and how the `group` package streams rows into grouped structures.
All paths repo-relative to `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core` unless stated otherwise (module prefix given when outside core).

## 1. Projections factory (`types/Projections.java`)

`Projections` is a static factory. Every method just `new`s a package-visible `FactoryExpression` subtype.
All target constructors of `QBean`/`QList`/`QMap`/`QTuple` are `protected`, so `Projections` is the only public entry point.

| Method | Returns | Target Java type requirement | Null / missing column behavior |
|---|---|---|---|
| `bean(Class, Expression...)` | `QBean<T>` | No-arg constructor + JavaBean setters matching path names (property access) | Per-property: `if (value != null) setter.invoke(...)`. Null column -> setter simply not called, field keeps default (usually null/0). No exception. |
| `bean(Class, Map<String,Expression>)` | `QBean<T>` | Same, but binding name is explicit map key, not the path name | Same null skip |
| `fields(Class, Expression...)` | `QBean<T>` (fieldAccess=true) | No-arg constructor + declared fields (any visibility, walks superclasses) matching path names, reflective `Field.set` | Same null skip, but via `Field.set` instead of setter |
| `constructor(Class, Expression...)` | `ConstructorExpression<T>` | A public constructor whose parameter types are assignable from the expr types (resolved via `ConstructorUtils.getConstructor`) | Null args passed straight into the constructor call; primitive params get transformed via `PrimitiveTransformer` (null -> default primitive value, e.g. `0`, `false`) so a null column into an `int` param does not NPE |
| `constructor(Class, Class[] paramTypes, Expression...)` | `ConstructorExpression<T>` | Same, but paramTypes given explicitly (needed when erasure/overload resolution is ambiguous) | Same |
| `tuple(Expression...)` | `QTuple` (`Expression<Tuple>`) | None - `Tuple` is a generic row wrapper (`TupleImpl`), no reflection at all | Null column value is simply stored at that index/expr key in the array; `get()` returns null |
| `map(Expression...)` | `QMap` (`Expression<Map<Expression<?>,?>>`) | None | `HashMap<Expression<?>, Object>`, null values allowed as map values |
| `list(Expression...)` | `QList` (`Expression<List<?>>`) | None | `Collections.unmodifiableList(Arrays.asList(args))` - nulls preserved as list elements |
| `array(Class<T[]>, Expression<T>...)` | `ArrayConstructorExpression<T>` | None (just needs component type) | Copies row array into a `T[]` array of matching component type; nulls preserved as array elements |
| `appending(Expression<T> base, Expression<?>... rest)` | `AppendingFactoryExpression<T>` | None | Serializes `base` + `rest` as SELECT columns (so extra columns are fetched, e.g. for use in ORDER BY/native filtering) but `newInstance` returns only `args[0]` (the base row value) - the rest are discarded at materialization time |
| implicit "constant" projection | not a `Projections` method - any `Expression` used directly as the select target (e.g. `ConstantImpl`, a single `Path`) is not wrapped in a `FactoryExpression` at all | N/A | The query engine returns the raw scalar/entity per row |

Usage snippets (patterns taken from the Javadoc in `types/Projections.java` and `types/QBean.java`/`ConstructorExpression.java`):

```java
// bean (setter based)
UserDTO dto = query.select(Projections.bean(UserDTO.class, user.firstName, user.lastName)).fetchOne();

// fields (reflective field access, bypasses setters)
UserDTO dto = query.select(Projections.fields(UserDTO.class, user.firstName, user.lastName)).fetchOne();

// constructor
UserDTO dto = query.select(Projections.constructor(UserDTO.class, user.firstName, user.lastName)).fetchOne();

// tuple (default projection for multi-column select())
List<Tuple> rows = query.select(Projections.tuple(user.firstName, user.lastName)).fetch();
rows.get(0).get(user.firstName);

// map (keyed by Expression, not String)
Map<Expression<?>, ?> row = query.select(Projections.map(user.firstName, user.lastName)).fetchOne();

// list
List<?> row = query.select(Projections.list(user.firstName, user.lastName)).fetchOne();

// array
Object[] row = query.select(Projections.array(Object[].class, user.firstName, user.lastName)).fetchOne();

// appending: select extra columns (e.g. for grouping key) but only materialize the base
Projections.appending(user, user.department); // returns `user` entity, but also selects department column
```

Bindings resolution detail (`types/QBean.java`, `createBindings`): a bare `Path` binds by `path.getMetadata().getName()`; an `Ops.ALIAS` operation (`expr.as(propertyName)`) binds by the alias path's name, letting you rename an arbitrary expression (including a nested `FactoryExpression` or `GroupExpression`) to a target property name - this is how nested-projection-as-a-bean-property works.
`QBean` validates type-compatibility at construction time (`isAssignableFrom`, with primitive/wrapper normalization) and throws `IllegalArgumentException` on mismatch; `propertyNotFound`/`typeMismatch` are `protected` no-op/throw hooks a subclass could override to be lenient.

There is also a non-standard, community-contributed `types/NameBasedProjection.java` (attributed to "Mouon" in the source) that reflectively matches an `EntityPathBase`'s field names to a DTO constructor's parameter names (falling back to declared-field order if `-parameters` was not used to compile). Not part of the documented public API surface (`Projections` has no factory method for it) but worth knowing about since it is shipped in `types/`.

## 2. `FactoryExpression` contract, composition, and column flattening

### Contract (`types/FactoryExpression.java`)

```java
public interface FactoryExpression<T> extends Expression<T> {
  List<Expression<?>> getArgs();      // the "columns" this projection needs, in order
  T newInstance(Object... args);      // build T from one materialized row segment
}
```

`getArgs()` is consulted twice: once to build the SQL/JPQL SELECT column list, once (per output row) to know how many/which values `newInstance` expects.
`FactoryExpressionBase` (`types/FactoryExpressionBase.java`) is the common abstract superclass all concrete projections extend; it implements `equals()` by class + args equality and adds `skipNulls()`, which wraps `this` in an anonymous `FactoryExpression` whose `newInstance` returns `null` if every argument is `null` (useful for a nested DTO populated from a `LEFT JOIN` that matched nothing - avoids materializing an all-null child object). `skipNulls()` is opt-in; nothing calls it automatically.

### Nesting / composition (`types/FactoryExpressionUtils.java`)

A projection can itself be an argument of another projection, e.g. `Projections.bean(OrderDTO.class, order.id, Projections.constructor(CustomerDTO.class, order.customer.id, order.customer.name).as("customer"))`.
Two operations make this work:

- `expand(List<Expression<?>> exprs)` - recursively walks the arg list; whenever an arg is itself a `FactoryExpression` (or a `ProjectionRole` wrapping one), it inlines that inner projection's own (recursively expanded) args instead of the projection object itself. This produces one **flat** list of leaf `Expression`s - this is what becomes the actual SQL `SELECT` column list.
- `compress(List<Expression<?>> exprs, Object[] args)` - the inverse, done per row at materialization time. It walks the *original* (unflattened) arg list; for a plain leaf expr it takes the next value from the flat `args` array; for a nested `FactoryExpression` it recursively slices out exactly `countArguments(fe)` consecutive values from the flat array, recursively compresses/materializes them into the nested object via `fe.newInstance(...)`, and puts that nested object into the result slot. `countArguments` mirrors `expand`'s recursion to know how many flat columns a nested projection consumed.

`FactoryExpressionUtils.wrap(FactoryExpression<T> expr)` builds a `FactoryExpressionAdapter<T>` only if any arg is itself a `FactoryExpression` (`wrap` short-circuits and returns `expr` unchanged otherwise - no overhead for flat projections). The adapter's `getArgs()` returns the expanded (flat) list; its `newInstance(Object... a)` calls `compress(inner.getArgs(), a)` to rebuild the nested `Object[]` tree, then delegates to `inner.newInstance(...)`.
`wrap(List<? extends Expression<?>>)` is a second overload used when the *whole select clause* (not just one factory expression) mixes plain columns and factory expressions - it wraps everything in an `ArrayConstructorExpression` first.

### Where this plugs into query execution

`support/QueryMixin.java` (`convert`, called from `setProjection`) is where a user's `.select(...)` argument gets wrapped: if it's a `FactoryExpression` (and not already an adapter) it is passed through `FactoryExpressionUtils.wrap`. So by the time the projection reaches serialization, `getArgs()` is guaranteed to be flat.
`querydsl-sql/.../SQLSerializer.java` (`select instanceof FactoryExpression ? ((FactoryExpression<?>) select).getArgs() : ...`) is where the flat arg list becomes the actual `SELECT col1, col2, ...` SQL text.
At the JDBC/JPA row level, materialization is a single call per row: `querydsl-jpa/.../TransformingIterator.java` and `FactoryExpressionTransformer.java` both just call `projection.newInstance((Object[]) row)` (wrapping a scalar row in a 1-element array first if the underlying driver returned a bare scalar instead of an array). All the nested-object reconstruction happens inside that one `newInstance` call via the `compress` logic described above - the query engine itself has no awareness of nesting.

### Notable concrete `FactoryExpression`s read

- `types/QBean.java` - reflective bean/field populate, described above.
- `types/ConstructorExpression.java` - resolves a `java.lang.reflect.Constructor` once at build time (`ConstructorUtils.getConstructor`), then applies a small pipeline of `Function<Object[],Object[]>` "transformers" (`util/ConstructorUtils.java`: `PrimitiveAwareVarArgsTransformer`, `PrimitiveTransformer`, `VarArgsTransformer`) before invoking the constructor - this is what lets a null DB value flow into a primitive constructor parameter (substituted with `0`/`false`/etc.) and what lets a varargs constructor be called from a fixed-arity expr list.
- `types/QTuple.java` - wraps an `Object[]` in a private `TupleImpl`; also builds a `Map<Expression<?>, Integer>` (`bindings`) at construction time so `Tuple.get(Expression)` is an O(1) lookup; an `Ops.ALIAS` operation's *aliased* expression is also registered as a lookup key.
- `types/ArrayConstructorExpression.java` - `newInstance` short-circuits and returns the incoming array unchanged if its component type already matches (avoids a copy); otherwise allocates a new correctly-typed array and `arraycopy`s.
- `types/AppendingFactoryExpression.java` (package-private, only reachable via `Projections.appending`) - `getArgs()` includes `base` + `rest`, but `newInstance` returns only `args[0]`; `accept()` delegates to `base` so the extra "rest" expressions never show up as a wrapping node.
- `types/MappingProjection.java` - abstract; user subclasses implement `map(Tuple row)`. Internally it just builds a `QTuple` from the constructor args (`ExpressionUtils.distinctList` dedupes if you accidentally repeat a column) and its own `newInstance` is `map(qTuple.newInstance(values))`. This is the escape hatch for arbitrary/custom row-to-object mapping without a JavaBean or constructor match.
- `types/QMap.java` / `types/QList.java` - trivial: `newInstance` builds a `HashMap<Expression<?>, Object>` / an unmodifiable `List` directly from the row values, keyed/ordered by the original expr list.
- `types/NameBasedProjection.java` - resolves a constructor by matching parameter names (or, if compiled without `-parameters`, by declared field order as a fallback) against an `EntityPathBase`'s field names; throws `RuntimeException` (not `IllegalArgumentException`) if no constructor/param can be satisfied.

## 3. `Tuple` (positional and expression-keyed access)

Interface `Tuple.java`:

```java
<T> T get(int index, Class<T> type);
<T> T get(Expression<T> expr);
int size();
Object[] toArray();
```

The only shipped implementation is `QTuple.TupleImpl` (private inner class of `types/QTuple.java`), backed by the raw `Object[] a` for that row.

- Positional access `get(index, type)` is an unchecked cast of `a[index]` - no bounds or type checking beyond the cast; `equals`/`hashCode`/`toString` all use `Arrays.*` over the backing array as `Tuple`'s own Javadoc mandates any implementation must.
- Expression-keyed access `get(expr)` looks `expr` up in `QTuple.bindings` (a `Map<Expression<?>, Integer>` built once at `QTuple` construction, shared by all `TupleImpl` rows from the same `QTuple`) and indexes into `a`; returns `null` if `expr` was never part of the projection (rather than throwing).
- `Ops.ALIAS` handling: if a select arg was `somePath.as(alias)`, both the original expr and `operation.getArg(1)` (the alias path) are registered against the same index, so `row.get(alias)` and `row.get(original)` both work.
- `Tuple` is also QueryDSL's *default* multi-column projection: `.select(e1, e2, ...)` without an explicit `Projections.*` call implicitly becomes `Projections.tuple(e1, e2, ...)` (see `QueryMixin.setProjection(Expression<?>... o)` -> `setProjection(Projections.tuple(o))`).

## 4. `group` package - the GroupBy DSL

### DSL entry points (`group/GroupBy.java`, `group/GroupByBuilder.java`)

```java
Map<Long, List<Tuple>> byDept =
    query.transform(
        GroupBy.groupBy(employee.department.id)
               .as(GroupBy.list(Projections.tuple(employee.id, employee.firstName))));
```

- `GroupBy.groupBy(Expression<K> key)` -> `GroupByBuilder<K>`. A multi-key overload `groupBy(Expression<?>... keys)` wraps the keys in `Projections.list(keys)` so `K` becomes `List<?>`.
- `GroupByBuilder<K>` terminal methods, each returning a `ResultTransformer<...>`:
  - `.as(Expression...)` / `.as(Supplier<Map>, Expression...)` -> `Map<K, Group>` (or a custom map impl), one `Group` per key holding *all* non-key columns.
  - `.as(Expression<V>)` / `.as(FactoryExpression<V>)` -> `Map<K, V>`, projecting the group down to a single value/DTO per key.
  - `.list(...)` -> `List<Group>` or `List<V>` (order = first-seen key order, values not sorted).
  - `.iterate(...)` -> `CloseableIterator<Group>` / `CloseableIterator<V>` - a **streaming**, non-buffering transform (see below).
  - `.collection(Supplier<RES>, ...)` -> arbitrary `Collection` (e.g. `HashSet::new`), via `GroupByGenericCollection`.
- Group-column wrapper expressions, each an `AbstractGroupExpression<T,R>` (implements `GroupExpression<T,R> extends Expression<R>`) with its own stateful `GroupCollector<T,R>`:

  | Factory | `group/*.java` | Collector semantics |
  |---|---|---|
  | `GroupBy.min(expr)` | `GMin` | running `Comparable` minimum |
  | `GroupBy.max(expr)` | `GMax` | running `Comparable` maximum |
  | `GroupBy.sum(expr)` | `GSum` | `BigDecimal` running sum, cast back to `T` via `MathUtils.cast` |
  | `GroupBy.avg(expr[, MathContext])` | `GAvg` | `BigDecimal` sum/count division at `.get()` time, default `MathContext.DECIMAL128` |
  | `GroupBy.list(expr)` | `GList` | `ArrayList`, appends every **non-null** value in row order |
  | `GroupBy.set(expr)` | `GSet.createLinked` | `LinkedHashSet`, skips null |
  | `GroupBy.sortedSet(expr[, Comparator])` | `GSet.createSorted` | `TreeSet` |
  | `GroupBy.map(keyExpr, valueExpr)` | `GMap.createLinked` (via `QPair`) | `LinkedHashMap`, later duplicate key **overwrites** (plain `map.put`) |
  | `GroupBy.sortedMap(keyExpr, valueExpr[, Comparator])` | `GMap.createSorted` | `TreeMap` |
  | plain `Expression` (no `GroupBy.*` wrapper) | implicit `GOne` | takes the value from the **first** row of the group only; every subsequent row's value for that column is silently ignored (not verified equal) |

  Nested grouping is supported via `MixinGroupExpression` (e.g. `GroupBy.list(GroupBy.map(...))` -> a list of maps, one map per distinct value of the outer group key that also matches the inner grouping) and `GMap.Mixin` (`GroupBy.map(GroupExpression, GroupExpression)`, e.g. grouping into a `Map<K, Map<K2,V>>`). Both work by delegating each incoming value to a *per-inner-key* nested `GroupCollector`, then flushing (`.get()`) each nested collector's result into the outer collector when the group is finalized.

### Row -> group streaming (`group/AbstractGroupByTransformer.java`, `group/GroupImpl.java`, `group/GroupCollector.java`)

`ResultTransformer<T>` (`core/ResultTransformer.java` - note: it lives in `com.querydsl.core`, not `core.types`) is a single-method SAM: `T transform(FetchableQuery<?,?> query)`. `FetchableQueryBase.transform(ResultTransformer<T>)` (`support/FetchableQueryBase.java`) just calls it with `this`.

`AbstractGroupByTransformer` constructor builds the actual flat SELECT projection: key expr first (wrapped `GOne`), then for each user expression either its `GroupExpression` as-is or an implicit `GOne` wrapper; the underlying plain `Expression`s (unwrapping `GroupExpression.getExpression()`, and unwrapping `Ops.ALIAS`) become the `expressions` array actually passed to `query.select(...)`.

Each concrete transformer (`GroupByMap`, `GroupByList`, `GroupByGenericMap`, `GroupByGenericCollection`, `GroupByIterate`) then does, at `transform(query)` time:

1. `FactoryExpressionUtils.wrap(Projections.tuple(expressions))` - build (and flatten) a `Tuple` projection over those columns; if any column is itself a `GroupExpression` (only relevant for the `GMap` key/value pair case), strip the wrapper via `withoutGroupExpressions` so the actual SQL select gets plain columns.
2. `query.select(expr).iterate()` - execute once, stream rows.
3. For each row: convert to a `Tuple` (`util/TupleUtils.toTuple`), take `row[0]` as the group key.
4. Look up (or create) a `GroupImpl` for that key and call `group.add(row)`, which feeds `row[i]` to the `i`-th `GroupCollector` (built once per `GroupImpl` from the `groupExpressions` list, matched by underlying `Expression` so multiple `GroupExpression`s over the same column share one collector, e.g. `GroupBy.list(x)` and `GroupBy.set(x)` over the *same* `x` would still each get their own collector since collector identity is keyed by `coldef.getExpression()` per distinct `GroupExpression` instance added - but the SQL/tuple column itself is only fetched once per distinct underlying `Expression`).
5. After exhausting all rows, `Group` objects are converted to the final `V`/`Map`/`List` via `Group.getOne`/`getList`/`getSet`/`getMap`/`getSortedMap`/`getGroup`, or (for the `FactoryExpression` overloads in `GroupByBuilder`) via `transformation.newInstance(args)` reading each non-key `groupExpressions` slot in order.

### Exact semantics

- **Order preservation**: `GroupByMap`/`GroupByGenericMap` accumulate into `LinkedHashMap` (or whatever `Supplier` the caller gave `GroupByGenericMap`/`GroupByGenericCollection`) keyed by the **first row's** value for that key - so map/list iteration order equals first-occurrence order of each distinct key in the underlying SQL result set, i.e. `ORDER BY` on the query still controls final group order as long as rows with the same key are contiguous or at least first-seen in the desired order. Within a group, `GList`/`GSet` (`LinkedHashSet`) preserve row-fetch order; `GSet.createSorted`/`GMap.createSorted` re-sort by `Comparable`/`Comparator`.
- **`GroupByList.transform`** literally builds the map first (`mapTransformer.transform(query)`) then returns `new ArrayList<>(result.values())` - so `.list(...)` is `.as(...)` (map) with the keys dropped, same order.
- **`GroupByIterate` (streaming)**: does **not** buffer the whole result set. It keeps exactly one `GroupImpl` "in flight"; `next()` keeps pulling rows and feeding the current group until it sees a row whose key `!Objects.equals(groupId, row[0])`, at which point it flushes the just-completed group and starts a new one. **This means `GroupByIterate` requires the underlying query's rows to already be ordered/clustered by the group key** (typically via `.orderBy(key)`) - if same-key rows are not contiguous, `iterate()` will silently produce multiple separate `Group`s for the same key instead of merging them. `GroupByMap`/`GroupByList`/`GroupByGenericCollection` (buffering, done with try-with-resources over the iterator) do *not* have this restriction because they look the key up in the whole-result map/track it in a running comparison, respectively - `GroupByGenericCollection` in fact has the *same* contiguous-key assumption as `GroupByIterate` (`!Objects.equals(groupId, row[0])` triggers a flush), only `GroupByMap`/`GroupByGenericMap` are truly order-independent (hash/tree map lookup by key).
- **Duplicate keys within a contiguous run**: merged into one `Group`/output value via each column's `GroupCollector` (list appends, set dedupes, sum/avg/min/max aggregate, map upserts, `GOne` keeps first-seen only).
- **`one()` vs `list()`**: there is no explicit "one()" method; a plain (unwrapped) `Expression` passed as a group column is implicitly `GOne`, i.e. "take the first row's value for this column and ignore the rest" - callers are expected to only do this for columns that are already known to be constant across the group (e.g. columns functionally dependent on the group key), QueryDSL does not verify or warn.
- `Group.getGroup(GroupExpression)` looks up by matching `GroupExpression.equals` (class + underlying expression), so calling `.getList(x)` on a `Group` that was built with `GroupBy.set(x)` (not `list`) throws `ClassCastException` (per Javadoc) rather than converting.

## 5. `@QueryProjection` and APT codegen

Annotation (`annotations/QueryProjection.java`): `@Target({CONSTRUCTOR, TYPE})`, fields `boolean useBuilder() default false`, `String builderName() default ""`.

APT processing (`querydsl-tooling/querydsl-apt/src/main/java/com/querydsl/apt/AbstractQuerydslProcessor.java`, `TypeElementHandler.java`):

- `processProjectionTypes` scans all elements annotated `@QueryProjection`. If it's on a constructor, the *enclosing class* becomes a registered "projection type" (`context.projectionTypes`), separate from `@Entity`-style types.
- `TypeElementHandler.handleConstructors` walks every constructor of the type; for each one that `configuration.isValidConstructor(...)` accepts, it reads the `@QueryProjection` annotation (if present) for `useBuilder`/`builderName`, validates (`useBuilder=true` requires non-empty `builderName`, and builder names must be unique per type), and records a codegen `Constructor` model with those flags plus the parameter list (name + resolved `Type`, including any `@QueryType` override).

Code generation (`querydsl-tooling/querydsl-codegen/src/main/java/com/querydsl/codegen/DefaultProjectionSerializer.java`):

- The generated `QXxx` type's superclass is emitted as `ConstructorExpression<Xxx>` (`new ClassType(TypeCategory.SIMPLE, ConstructorExpression.class, model)`), i.e. `@QueryProjection` reuses the exact same runtime class as `Projections.constructor(...)` - there is no separate `QConstructor` runtime type.
- For each `@QueryProjection` constructor, it emits a matching public constructor in `QXxx` whose parameter types are `Expression<? extends P>` for each original parameter type `P` (`getExpressionParameter`, `asExpr` flag dedupes when two constructors share the same arity so the non-Expression-typed convenience overload isn't duplicated), whose body is:
  ```java
  super(Xxx.class, new Class<?>[]{ P1.class, P2.class, ... }, arg1, arg2, ...);
  ```
  i.e. it directly calls the three-arg `ConstructorExpression` super constructor with the **concrete parameter types statically known from the DTO's own constructor**, not types inferred from whatever `Expression`s the caller happens to pass.
- This is what makes it "type-safe" versus `Projections.constructor(...)`: `Projections.constructor` accepts `Expression<?>...` and only checks constructor-compatibility at **runtime** (`ConstructorUtils.getConstructor`, throwing `IllegalArgumentException` if no matching constructor is found), whereas `new QXxx(expr1, expr2)` is a **compile-time** checked Java constructor call - passing an `Expression<Integer>` where `Expression<? extends String>` is expected fails to compile.
- Optional builder support (`useBuilder=true`): generates a static inner `XxxBuilder` class with one private `Expression<...>` field + fluent `setXxx(...)` setter per constructor param, a `build()` method that calls `new QXxx(field1, field2, ...)`, and a static factory `QXxx.builderXxx()` (name derived from `builderName`) to start the chain - lets callers build the projection with named setters instead of positional args.

## 6. Design recommendations for the Rust port

| Java concept | Rust mapping | Rationale / concrete shape |
|---|---|---|
| `Projections.constructor` / `@QueryProjection` | **Not needed as a runtime-reflective mechanism.** Replace with a `#[derive(FromRow)]`-style derive macro (analogous to `sqlx::FromRow`) generating `fn from_row(row: &[Value]) -> Self` (or `TryFrom<&[Value]>`) that reads fields positionally in declaration order. | Rust has no runtime reflection and no method overload resolution to replicate `ConstructorUtils.getConstructor`'s runtime matching; a derive macro gives the same "type-safe constructor projection" QueryDSL achieves via generated `QXxx` types, but at Rust's normal compile boundary (proc-macro expansion) instead of a separate APT pass. Field/column order is the contract, exactly like the generated `QXxx(Class<?>[] paramTypes, ...)` call. |
| `Projections.bean` / `Projections.fields` | **Skip.** No JavaBean/reflective setter convention exists in idiomatic Rust, and reflection-based field writes are not available without `unsafe` transmutation. | The derive-macro struct projection above already covers "populate a struct from a row"; there is no separate use case for setter-vs-constructor population in Rust since structs are usually constructed atomically. If partial/mutable population is wanted, `Default + field assignment` can be generated instead as a derive-macro variant, but this should be a flag on the same macro, not a separate mechanism. |
| `Projections.tuple` / `Tuple` interface | **Native Rust tuples + a small trait**, e.g. `impl<A, B> FromRow for (A, B) where A: FromColumn, B: FromColumn`, implemented via macro for arities up to ~16 (mirrors `diesel`/`sqlx` patterns). Expression-keyed lookup (`Tuple.get(Expression)`) is unnecessary. | Rust's tuple types already give positional, statically-typed access with zero runtime cost - no need for `QTuple`'s `Map<Expression<?>, Integer>` binding table. Expression-keyed lookup existed in Java because `Expression<T>` objects are compared by identity/structural equality at runtime; in a Rust query builder the same information is known at compile time (the same `Expression` value used in `.select()` is the same Rust value/type used to destructure the tuple), so keep it purely positional. |
| `Projections.array` | **Skip** as a distinct feature; a fixed-size array `[T; N]` projection falls out for free once the tuple-family macro exists (or just don't support it - `Vec<T>`/tuple covers the need). | Java needed a typed-array projection because Java generics can't express `T[]` well without reflection (`Array.newInstance`); Rust arrays are a first-class generic-friendly type, so this is not a distinct problem to solve. |
| `Projections.map` (`QMap`) | **Low priority.** If needed, `HashMap<ColumnId, Value>` where `ColumnId` is whatever the query builder uses as an untyped column handle (e.g. an enum or `&'static str`), built the same way (`newInstance` just `zip`s columns with values). | Keep for dynamic/reflection-like use cases (e.g. exporting a row to JSON) but do not make it the default multi-column projection like `Tuple` is in QueryDSL. |
| `Projections.appending` | **Skip**, or replace with a plain "select extra column, ignore it in the projected type" pattern expressed by the derive macro accepting `#[querydsl(skip)]`-style field markers, or simply: let callers `.select((real_projection, extra_col))` (a 2-tuple) and discard `.1` themselves. | The Java feature exists to smuggle extra SELECT columns (e.g. for `ORDER BY` on an aggregate not otherwise selected) past a single-value projection. Rust's tuple projection already gives this for free (`(MyDto, i64)` selects both and the caller drops the second field), no dedicated wrapper type needed. |
| `MappingProjection` | **Keep, as the generic escape hatch.** A trait `trait RowMapper<T> { fn map(&self, row: &TupleRow) -> T; }` or simply allowing `.select(...).map(|row| ...)` (a closure taking the tuple-projection row) covers arbitrary custom mapping without inventing bean/setter machinery. | This is QueryDSL's "if none of the structured projections fit, write it yourself" hatch; a closure over the already-typed Rust tuple is strictly simpler than Java's `Tuple` abstraction because the tuple is already statically typed. |
| Nested `FactoryExpression` composition + column flattening (`FactoryExpressionUtils.expand`/`compress`) | **Push this to the type system / derive macro at compile time instead of a runtime flatten/unflatten pass.** Each projection type (including a nested struct field) implements a trait exposing (a) its flat list of columns (`fn columns() -> Vec<ColumnRef>` or a `SELECT_COLUMNS: &'static [...]` const) and (b) `from_row_slice(&[Value]) -> Self` that consumes exactly `Self::COLUMN_COUNT` values, recursing into nested projections' own `from_row_slice` on sub-slices. The derive macro generates both, walking field types to detect "this field is itself a projection" (e.g. via a marker trait) the same way `FactoryExpressionUtils.expand` detects `arg instanceof FactoryExpression`. | Java has to do this at runtime with `Object[]` and reflection-free but type-erased `getArgs()`/`newInstance(Object...)` because there is one shared `Expression`/`FactoryExpression` object graph shape for every projection kind. Rust's monomorphized generics can generate the equivalent flatten/rebuild logic per concrete type at compile time (each `T: FromRow` impl knows its own arity and layout as an associated const/fn), which is both faster (no `Object[]` boxing, no recursive `countArguments` walk per row) and safer (mismatched column counts become type errors or panics, not `ArrayIndexOutOfBoundsException`). This is the single most important structural difference to design for - "flatten nested projection args into one SELECT list, then unflatten per row" is exactly the problem const-sized/derive-computed arity solves at zero runtime cost. |
| `skipNulls()` (`FactoryExpressionBase.skipNulls`) | **Keep as an opt-in derive attribute or wrapper combinator**, e.g. `#[derive(FromRow)] #[querydsl(skip_if_all_null)] struct ChildDto {...}` generating `from_row_slice` returning `None`/skip when every consumed value is null - or more idiomatically, make nested-optional projections `Option<ChildDto>` and have the macro emit exactly this null-check automatically for any field typed `Option<T: FromRow>`. | Rust's `Option<T>` already expresses "this LEFT JOIN branch may not have matched" more naturally than Java returning a nullable, all-default-value bean; prefer wiring `skipNulls` semantics into `Option<T>` field detection rather than an explicit opt-in call. |
| `group` package (`GroupBy`, `Group`, `GroupExpression`, `GAvg`/`GSum`/.../`GMap`) | **Replace the whole runtime `GroupCollector`/`Group`-lookup-by-equals machinery with a streaming iterator adapter over already-typed tuple rows**, e.g. `rows.into_iter().chunk_by(|r| r.key)` (via `itertools::Itertools::chunk_by`, mirroring the `GroupByIterate` contiguous-key assumption) or a buffering `group_by_key(rows, key_fn) -> HashMap<K, Vec<V>>`/`IndexMap<K, Vec<V>>` (mirroring `GroupByMap`'s `LinkedHashMap`, order-independent). Aggregation "collectors" (`min`/`max`/`sum`/`avg`/`list`/`set`) map directly onto `Iterator::min`/`max`/`sum`/`fold`/`collect::<Vec<_>>`/`collect::<HashSet<_>>` applied per group - no need for a `GroupCollector` trait object per column, since Rust iterator adapters already compose this generically and statically. | Java needed `Group`/`GroupCollector`/`GroupExpression` as a runtime object graph because it has to support an open-ended, dynamically-composed set of "what does this column become" (`GList` vs `GSet` vs `GSum`...) resolved by identity-equality lookup at runtime (`GroupImpl.groupCollectorMap`). In Rust, "what do I do with this column per group" is just "what iterator combinator do I call after `chunk_by`/grouping" - a normal, statically-typed, zero-cost expression, not a class hierarchy. Preserve the *documented semantics* precisely (this is the part worth keeping 1:1, see below), not the class structure. |
| `GroupBy.groupBy(key).as(...)`, `Map<K,V>`/`List<V>` results, order preservation, `GOne` "first value wins", contiguous-key requirement for `iterate()` | **Keep the semantics exactly**, since they are non-obvious and easy to get subtly wrong (esp. the "buffering transforms are order-independent by key, `iterate()` is not" split). Document/test explicitly: (1) buffered group-by (`HashMap`/`IndexMap` keyed accumulate) tolerates any row order; (2) a true streaming/chunking group-by requires the input already sorted/clustered by key, and must be named/typed distinctly (e.g. `GroupByIter` vs `GroupBy`) so callers can't accidentally use the streaming one on unsorted input; (3) a bare (non-aggregated) column defaults to "first row's value, later rows ignored" - make this an explicit combinator name like `.first()` rather than an implicit default, to avoid QueryDSL's silent-if-inconsistent behavior. | These are the actual value QueryDSL's `group` package provides (a well-tested, subtle streaming/grouping contract); the Java class hierarchy that implements them is not something to port, but the contract is exactly what a Rust user reaching for this feature needs preserved. |
| `Fetchable`/`ResultTransformer` (`core/Fetchable.java`, `core/ResultTransformer.java`) | **Map onto a `Stream`/`Iterator`-first query execution API** (`fn fetch(self) -> Vec<T>`, `fn stream(self) -> impl Iterator<Item = T>`, `fn fetch_one(self) -> Option<T>`), with grouping/transformation expressed as ordinary iterator adapters chained after `stream()` rather than a `transform(ResultTransformer<T>)` indirection. | `ResultTransformer` exists in Java partly to let `GroupBy` hook into `query.transform(...)` without `Fetchable` knowing about grouping; in Rust, since `GroupBy` becomes "an iterator adapter," it composes with `.stream()` directly (`query.stream().group_by_key(...)`), so the separate `ResultTransformer` seam is unnecessary abstraction. |

### Summary of what NOT to port 1:1

- No runtime reflection layer (`QBean`'s setter/field introspection, `ConstructorUtils`'s constructor-matching-by-assignability, `NameBasedProjection`'s name matching) - all replaced by compile-time derive macro expansion.
- No `Object[]`-based row representation with runtime `expand`/`compress` flattening - replaced by compile-time-known arity per projection type.
- No `Expression`-keyed `Tuple` lookup - Rust tuples are already statically positioned/typed.
- No `GroupCollector`/`Group` class hierarchy - replaced by standard iterator combinators after a `chunk_by`/keyed-group adapter.
- No separate `@QueryProjection` + APT + codegen pipeline for "type-safe constructor projection" - a single derive macro on the target struct achieves the same compile-time guarantee without a parallel `QXxx` type or annotation processor pass.

### What to port faithfully (behavioral contracts worth copying exactly)

- Null-column handling per projection kind (struct-projection: leave default/`Option::None`; tuple/list/map: null value preserved; primitive constructor param: substitute type default only if the Java precedent of "never panic on a nullable numeric column into a non-Option field" is a design goal you want to keep - otherwise consider making this a hard compile-time-or-runtime error in Rust, which is arguably better than QueryDSL's silent zero-substitution).
- `GroupBy` order-preservation and first-seen-key ordering.
- The buffered-vs-streaming group-by distinction and its contiguous-key precondition for the streaming variant.
- `GOne`/"first value wins, not verified consistent" semantics for un-aggregated group columns, made explicit rather than implicit.

## Unresolved questions

- Should the Rust port support a `Projections.map`-equivalent at all, given it has the weakest ergonomics of the Java projections and Rust already has strongly-typed alternatives (tuple/struct) for every case it covers.
- Should null-in-non-nullable-column be a panic, a compile-time-unreachable state (via `NOT NULL` schema introspection), or a silent default substitution (matching Java) - this is a product decision about strictness vs Java-parity, not something the QueryDSL source resolves for us.
- Whether the streaming (`GroupByIterate`-equivalent) grouping adapter is worth building at all in v1, given it has a sharp footgun (silently wrong on unsorted input) that the buffered version does not have.

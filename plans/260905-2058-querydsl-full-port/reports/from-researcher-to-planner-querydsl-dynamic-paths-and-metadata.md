# QueryDSL Dynamic Query Construction, Path Model, and Escape Hatches

Source root for all citations: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/` unless stated otherwise.
All paths below are relative to the querydsl repo root.

## 1. BooleanBuilder

File: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/BooleanBuilder.java`

`BooleanBuilder` is a mutable, cascading `Predicate` builder.
It wraps a single nullable `Predicate predicate` field and implements `Predicate` itself (so it can be passed anywhere a predicate is expected, including nested inside another `BooleanBuilder`).

Exact semantics:

| Method | Behavior when arg is null | Behavior when arg is non-null and builder is empty | Behavior when arg is non-null and builder is non-empty |
|---|---|---|---|
| `and(Predicate right)` | no-op, returns `this` unchanged | `predicate = right` | `predicate = ExpressionUtils.and(predicate, right)` |
| `or(Predicate right)` | no-op | `predicate = right` | `predicate = ExpressionUtils.or(predicate, right)` |
| `andNot(Predicate right)` | delegates to `and(right.not())` (NPE if `right` is null, no null guard here) | same as `and` | same as `and` |
| `orNot(Predicate right)` | same NPE risk | same as `or` | same as `or` |
| `andAnyOf(Predicate... args)` | empty varargs is a no-op | `and(ExpressionUtils.anyOf(args))`, i.e. `this && (arg1 || arg2 || ... || argN)` | same, ANDed onto existing state |
| `orAllOf(Predicate... args)` | empty varargs is a no-op | `or(ExpressionUtils.allOf(args))`, i.e. `this || (arg1 && arg2 && ... && argN)` | same, ORed onto existing state |
| `not()` | if `predicate == null`, stays null (no-op) | n/a | `predicate = predicate.not()` in place |
| `hasValue()` | returns `predicate != null` | | |
| `getValue()` | returns the current `Predicate` or null | | |

Key null-tolerance property: `and`/`or` silently ignore a null argument instead of throwing or short-circuiting to `FALSE`/`TRUE`.
This is exactly what makes it useful for dynamic filters built from optional user-supplied criteria (a null means "this filter criterion was not supplied," not "this filter criterion is a false predicate").

Constructor `BooleanBuilder(Predicate initial)` calls `ExpressionUtils.extract(initial)`, which visits the given expression through `ExtractorVisitor` (`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/ExtractorVisitor.java`).
Since `BooleanBuilder.accept()` recursively delegates to its wrapped `predicate.accept()` (or returns null if empty), `extract()` effectively unwraps a `BooleanBuilder`-wrapped-in-a-`BooleanBuilder` down to the real underlying `Operation`/`Path`/`Constant`/etc., or to `null` if the inner builder was empty.
This keeps the tree from accumulating builder wrapper nodes.

`equals`/`hashCode`/`toString` all delegate to the wrapped `predicate` (or `super.toString()` if null), and `clone()` is a shallow `Object.clone()` (the wrapped `Predicate` tree is immutable, so a shallow copy is safe for cloning the builder itself).

Why this pattern exists: QueryDSL predicates (`Predicate`, `Operation`, `Path`, etc.) are immutable value types (`@Immutable` on most impl classes).
Composing an arbitrary, runtime-determined number of AND/OR terms (e.g. "search form with 6 optional fields") with immutable trees would otherwise require null-checking boilerplate before every `ExpressionUtils.and/or` call at every call site.
`BooleanBuilder` centralizes that null-check once, inside a stateful accumulator, and is deliberately the *only* mutable `Expression` implementation in the type system.

Canonical usage (from `docs/guides/creating-queries.md`):

```java
public List<Customer> getCustomer(String... names) {
    QCustomer customer = QCustomer.customer;
    JPAQuery<Customer> query = queryFactory.selectFrom(customer);
    BooleanBuilder builder = new BooleanBuilder();
    for (String name : names) {
        builder.or(customer.name.eq(name));
    }
    query.where(builder); // customer.name eq name1 OR customer.name eq name2 OR ...
    return query.fetch();
}
```

The more common "optional filter" idiom elsewhere in the codebase and ecosystem:

```java
BooleanBuilder where = new BooleanBuilder();
if (nameFilter != null) {
    where.and(customer.name.eq(nameFilter));
}
if (minAge != null) {
    where.and(customer.age.goe(minAge));
}
query.where(where); // where.hasValue() == false emits no WHERE clause at all
```

Note `query.where(predicate)` itself is also null-tolerant at the `QueryMetadata` level (see section 4): `DefaultQueryMetadata.addWhere` calls `ExpressionUtils.extract(e)` and skips entirely if the result is null, so passing an empty `BooleanBuilder` (whose `accept()` returns null) or a raw `null` both correctly produce "no WHERE clause," not a "WHERE true" or NPE.

## 2. ExpressionUtils static helpers

File: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/ExpressionUtils.java`

This class is documented in its own Javadoc as "used internally in Querydsl and is not suitable to be used in cases where DSL methods are needed" — it builds *minimal internal* `Expression` implementations, not the richer fluent `dsl.*Expression` wrappers `Expressions` returns.

| Method | Signature (abridged) | Purpose |
|---|---|---|
| `operation` | `Operation<T> operation(Class<T>, Operator, Expression<?>...\|List)` | Generic operation node; returns `PredicateOperation` if `type == Boolean.class`, else `OperationImpl` |
| `predicate` | `PredicateOperation predicate(Operator, Expression<?>...\|List)` | Boolean-typed operation node |
| `path` | `Path<T> path(Class<T>, String variable)` / `(Class<T>, Path<?> parent, String property)` / `(Class<T>, PathMetadata)` | Creates a raw `PathImpl` |
| `predicateTemplate` / `template` | various overloads | String-template-based expressions (`PredicateTemplate` / `TemplateExpressionImpl`, boolean-typed collapses to `PredicateTemplate`) |
| `all(CollectionExpression)` / `all(SubQueryExpression)` | | `ALL` quantifier operation |
| `any(CollectionExpression)` / `any(SubQueryExpression)` | | `ANY` quantifier operation |
| `allOf(Collection<Predicate>\|Predicate...)` | returns `@Nullable Predicate` | Left-folds `and()` over non-null args; returns null if all are null/empty |
| `anyOf(Collection<Predicate>\|Predicate...)` | returns `@Nullable Predicate` | Left-folds `or()` over non-null args |
| `and(Predicate, Predicate)` | | Extracts both sides first; if either is null returns the other; else `predicate(Ops.AND, left, right)` |
| `or(Predicate, Predicate)` | | Same pattern with `Ops.OR` |
| `as(Expression, Path\|String)` | | `Ops.ALIAS` operation (`source AS alias`) |
| `count(Expression)` | | `Ops.AggOps.COUNT_AGG` |
| `eq` / `eqConst` | | `Ops.EQ`; `eqConst` wraps rhs constant via `ConstantImpl.create` |
| `ne` / `neConst` | | `Ops.NE` |
| `in(Expression, Collection\|CollectionExpression\|SubQueryExpression)` | | `Ops.IN`; **single-element `Collection` short-circuits to `eqConst`** |
| `inAny(Expression, Iterable<Collection>)` | | Builds `BooleanBuilder` ORing `in(left, list)` for each list |
| `notIn` / `notInAny` | | Mirror of `in`/`inAny` with `Ops.NOT_IN`, ANDed for `notInAny` |
| `isNull` / `isNotNull` | | `Ops.IS_NULL` / `Ops.IS_NOT_NULL` |
| `likeToRegex` / `regexToLike` | | Bidirectional SQL-LIKE <-> Java-regex pattern conversion (constant-folds when arg is a `Constant`, recurses through `Ops.CONCAT`) |
| `list(Class, Expression...\|List)` | | Builds a chained `Ops.SINGLETON`/`Ops.LIST` operation tree for tuple/list expressions |
| `distinctList(Expression...)` | | Dedupes an expression array preserving order |
| `extract(Expression)` | | Unwraps wrapper expressions via `ExtractorVisitor`; short-circuits identity for `PathImpl`/`PredicateOperation`/`ConstantImpl` |
| `createRootVariable(Path, int suffix)` / `createRootVariable(Path)` | | Synthesizes a fresh alias name from a path's string form (see section 4) |
| `toExpression(Object)` | | Wraps a plain object as `ConstantImpl` unless already an `Expression` |
| `toLower(Expression<String>)` | | Constant-folds `String.toLowerCase()`, else `Ops.LOWER` operation |
| `orderBy(List<OrderSpecifier<?>>)` | | Wraps order list as an `Ops.ORDER` operation (used by window-function `OVER (ORDER BY ...)` clauses) |

## 3. Expressions factory catalog

File: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/dsl/Expressions.java` (1962 lines).

Unlike `ExpressionUtils`, this is the *public*, richer factory: it returns the fluent `dsl.*Expression` types (which support `.and()`, `.eq()`, `.gt()`, etc.) rather than minimal internal nodes.
Grouped catalog (representative overloads only; most have 3-4 overloads for `String template`/`Template`/with-or-without `Class` param):

| Group | Representative methods | Returns |
|---|---|---|
| Constants | `ONE`, `TWO`, `THREE`, `FOUR`, `ZERO`, `TRUE`, `FALSE` (static fields) | `NumberExpression<Integer>` / `BooleanExpression` |
| Aliasing | `as(Expression, Path\|String)` | `SimpleExpression<D>` |
| Current time | `currentDate()`, `currentTime()`, `currentTimestamp()` | `DateExpression`/`TimeExpression`/`DateTimeExpression` |
| Boolean combinators | `allOf(BooleanExpression...)`, `anyOf(BooleanExpression...)` | `BooleanExpression`, folds via `.and()`/`.or()`, null-tolerant per element |
| Constants | `constant(T)`, `constantAs(D, Path<D>)` | `Expression<T>` / `SimpleExpression<D>` |
| Templates | `template`, `simpleTemplate`, `dslTemplate`, `comparableTemplate`, `dateTemplate`, `dateTimeTemplate`, `timeTemplate`, `enumTemplate`, `numberTemplate`, `stringTemplate`, `booleanTemplate` | matching `*Template` type, for raw string-template expressions (escape hatch, see section 9) |
| Operations | `operation`, `simpleOperation`, `dslOperation`, `predicate`, `booleanOperation`, `comparableOperation`, `dateOperation`, `dateTimeOperation`, `timeOperation`, `numberOperation`, `stringOperation`, `enumOperation` | matching `*Operation` type |
| Paths | `path`, `simplePath`, `dslPath`, `comparablePath`, `comparableEntityPath`, `datePath`, `dateTimePath`, `timePath`, `numberPath`, `stringPath`, `booleanPath`, `enumPath` | matching `dsl.*Path` type, built from `(Class, variable)` / `(Class, parent, property)` / `(Class, PathMetadata)` |
| Collections/tuples | `list(Class, ...)`, `list(...)` (returns `Tuple`), `set(Class, ...)`, `set(...)` | `SimpleExpression<T>` / `Expression<Tuple>` |
| Null | `nullExpression()`, `nullExpression(Class)`, `nullExpression(Path)` | `NullExpression<T>` |
| Collection-typed paths | `collectionOperation`, `collectionPath`, `listPath`, `setPath`, `mapPath`, `arrayPath` | matching collection path/operation type |
| Case | `cases()` | `CaseBuilder` |
| Type coercion | `asBoolean`, `asComparable`, `asDate`, `asDateTime`, `asTime`, `asEnum`, `asNumber`, `asString`, `asSimple` (each with `Expression<T>` and raw-value `T` overloads) | Wraps an arbitrary `Expression`/value into the matching fluent DSL type without needing a generated Q-type |

Practical rule stated in the docs (`docs/guides/creating-queries.md`): use `Expressions` only when the fluent Q-type DSL is unavailable — dynamic paths, custom syntax, or custom operations.

## 4. The Path model

Files:
`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/Path.java`,
`PathMetadata.java`, `PathMetadataFactory.java`, `PathType.java`, `PathImpl.java`, `Templates.java`.

`Path<T> extends Expression<T>` adds three members: `getMetadata()`, `getRoot()`, `getAnnotatedElement()`.

`PathMetadata` (immutable, `final class`) holds:
- `element: Object` — the property name (`String`) for `PROPERTY`/`VARIABLE`, or an index/key `Expression`/constant for indexed access types.
- `parent: Path<?>` (nullable) — the parent path; null only for a root `VARIABLE`.
- `pathType: PathType`.
- `rootPath: Path<?>` — cached from `parent.getRoot()` at construction time, or null if this *is* the root.
- `isRoot()` returns true if `parent == null`, or if `pathType == DELEGATE` and the delegate's own parent is root (so a `BeanPath.as(subtype)` cast delegate is treated as root-equivalent).

`PathType` (`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/PathType.java`) enumerates the relation of a path to its parent, and doubles as an `Operator` (used as a template-lookup key):

| PathType | Meaning | Serialization template (`Templates.java`) |
|---|---|---|
| `VARIABLE` | Root path (query alias) | `{0s}` (raw string, the alias) |
| `PROPERTY` | `parent.property` | `{0}.{1s}` |
| `COLLECTION_ANY` | Any-element access on a collection (`collection.any()`) | `any({0})` |
| `LIST_FIRST` | `list.getFirst()` | `{0}.getFirst()` |
| `LISTVALUE` | Indexed list access with expression index (`list.get(expr)`) | `{0}.get({1})` |
| `LISTVALUE_CONSTANT` | Indexed list access with literal int index | `{0}.get({1s})` (serialized constant, i.e. inlined not bound) |
| `ARRAYVALUE` | Array access with expression index | `{0}[{1}]` |
| `ARRAYVALUE_CONSTANT` | Array access with literal int index | `{0}[{1s}]` |
| `MAPVALUE` | Map access with expression key | `{0}.get({1})` |
| `MAPVALUE_CONSTANT` | Map access with literal key | `{0}.get({1})` (same template as MAPVALUE, key stored as constant element) |
| `DELEGATE` | Wraps another path (used for `BeanPath.as(subtype)` casts) | `{0}` (transparent passthrough) |
| `TREATED_PATH` | JPA `TREAT` downcast marker | (subsystem-specific, not in base `Templates`) |

`PathMetadataFactory` (`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/PathMetadataFactory.java`) is the sole constructor surface: `forVariable`, `forProperty`, `forCollectionAny`, `forListFirst`, `forListAccess` (expr and int-literal overloads), `forArrayAccess` (same), `forMapAccess` (expr-key and literal-key overloads), `forDelegate`.

**Nested paths (`a.b.c`)**: each level is a new `PathImpl`/`BeanPath` node whose `PathMetadata.parent` points at the previous node.
There is no flattened string representation stored anywhere — the dotted form only appears when a `Visitor` (typically `ToStringVisitor`, `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/ToStringVisitor.java`) walks the parent chain and applies the `PathType.PROPERTY` template `"{0}.{1s}"` recursively, bottom-up, at serialization time.
Backend-specific serializers (JPQLSerializer, SQLSerializer) apply their own templates on top for column-qualification, joins, etc.

**Root variable aliasing**: a root path is created via `PathMetadataFactory.forVariable(String)`, which the caller supplies explicitly (e.g. `new QCustomer("customer")`, or `PathBuilder<>(Customer.class, "customer")`).
There is no automatic uniqueness guarantee at construction time.
When the query engine needs to synthesize a *new*, guaranteed-fresh alias for a rewritten subexpression (e.g. turning `person.addresses.any()` into a JPQL `INNER JOIN person.addresses addresses_123` because JPQL has no native "any element" path syntax), it calls `ExpressionUtils.createRootVariable(Path, int suffix)` (or the no-suffix overload), which stringifies the original path via `ToStringVisitor` with an underscore-joining `Templates` variant (`UnderscoreTemplates`, private to `ExpressionUtils`) and appends `"_" + suffix`.
Concrete call sites: `CollectionAnyVisitor`, `JPACollectionAnyVisitor`, `JPAListAccessVisitor`, `JPAMapAccessVisitor`, `JPAQueryMixin` (all under `querydsl-libraries/querydsl-core/.../support/` and `querydsl-libraries/querydsl-jpa/.../`) — these rewrite `COLLECTION_ANY`/`LISTVALUE`/`MAPVALUE` path expressions into an added `JOIN` plus a freshly-aliased root path, since those backends can't express "any element of a collection" as a plain path.

**Validation of path usage**: `DefaultQueryMetadata.setValidate(true)` turns on `ValidatingVisitor` (`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/ValidatingVisitor.java`), which walks every expression added to `where`/`join`/projection and throws `IllegalArgumentException("Undeclared path '%s'...")` if a path's root was never registered via `addJoin`/`from`.
This is opt-in (default `validate = false` in `DefaultQueryMetadata`).

## 5. PathBuilder: dynamic, string-keyed path access

Files: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/dsl/PathBuilder.java`, `PathBuilderFactory.java`, `PathBuilderValidator.java`.

`PathBuilder<T> extends EntityPathBase<T>` is the fully dynamic, string-keyed, type-unsafe escape hatch for constructing paths when no generated Q-type exists (e.g. metaprogramming, generic repository code, dynamically-loaded entity metadata).

API surface: `get(String property)` (returns `PathBuilder<Object>`, generic), `get(String property, Class<A> type)` (typed), plus typed convenience methods mirroring every concrete path type: `getString`, `getBoolean`, `getNumber`, `getComparable`, `getDate`, `getDateTime`, `getTime`, `getEnum`, `getSimple`, `getArray`, `getCollection`, `getList`, `getSet`, `getMap` — each returning the matching `dsl.*Path`/`dsl.*Path`-parameterized-collection type instead of another `PathBuilder`.
There are also `get(ExistingPathInstance)` overloads (e.g. `get(StringPath path)`) that re-derive a typed child path from an already-built Q-type path's last path element name, and additionally copy over any `EntityPath.getMetadata(Path)` side-metadata (JPA `@QueryInit`-style annotations) the parent path knew about, via `addMetadataOf`.

Each `PathBuilder` caches its child `PathBuilder<Object>`/`<A>` instances in a `ConcurrentHashMap<SimpleEntry<String,Class<?>>, PathBuilder<?>>` keyed by `(property, type)`, so repeated `get("x")` calls return the same instance (structural identity matters because `Path.equals()` compares `PathMetadata`, which itself does an `Objects.equals(parent, ...)` chain up to root).

Canonical usage (from `PathBuilder.java` Javadoc and `docs/guides/creating-queries.md`):

```java
PathBuilder<User> user = new PathBuilder<User>(User.class, "user");
Predicate filter = user.getString("firstName").eq("Bob");
List<User> users = query.from(user).where(filter).select(user).fetch();
```

`PathBuilderFactory` (`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/dsl/PathBuilderFactory.java`) is a tiny cache keyed by `Class<?>`, auto-deriving the root variable name via `StringUtils.uncapitalize(type.getSimpleName()) + suffix` — i.e. `Customer.class` -> variable `"customer"`.

**Validation** (`PathBuilderValidator`, `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/dsl/PathBuilderValidator.java`) is a pluggable `validate(Class parent, String property, Class propertyType) -> Class<?>` hook invoked on every `get*` call, before the child path node is constructed. Three built-in strategies:

| Validator | Behavior | Real type-safety? |
|---|---|---|
| `DEFAULT` | Only rejects property names containing whitespace (`throw new IllegalStateException("Unsafe due to CVE-2024-49203")`); otherwise returns `propertyType` unchanged | **No** — any non-whitespace string is accepted regardless of whether the property exists |
| `FIELDS` | Walks `parent`'s class hierarchy via reflection (`getDeclaredField`), unwraps `Map`/`Collection` generic parameter types, else wraps primitives | Yes, against declared fields |
| `PROPERTIES` | Uses `BeanUtils.getAccessor("get"/"is", property, parent)` (JavaBean getter reflection), same generic unwrapping | Yes, against bean accessors |

There is also `JPAPathBuilderValidator` (`querydsl-libraries/querydsl-jpa/src/main/java/com/querydsl/jpa/support/JPAPathBuilderValidator.java`), which validates against a live JPA `Metamodel` (`entityManager.getMetamodel().managedType(parent).getAttribute(property)`), correctly resolving plural-attribute element types.

**Important security nuance for the CVE reference**: `DEFAULT`'s whitespace check exists specifically to block a known injection vector (CVE-2024-49203) where a crafted property name containing a space could break out of the generated path/alias fragment when serialized into JPQL/HQL string form.
It is *not* a general allowlist — it does not verify the property is real, so `PathBuilder` with the default validator is still "type-unsafe" in the sense the task asks about: it will happily build a path to a nonexistent property (surfacing only as a runtime error from the backend, e.g. "unknown column"), and it will build a syntactically-valid but semantically wrong path if given an unexpected but space-free string, but it does block the specific string-escape attack that made it a CVE.

What it enables: PathBuilder lets code operate on entity properties known only at runtime (e.g. generic search/filter frameworks, reflection-driven admin UIs, dynamically configured report builders) without generated Q-classes, at the cost of losing compile-time property-name and type checking.

## 6. Param / Constant

Files: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/ParamExpression.java`, `ParamExpressionImpl.java`, `ParamNotSetException.java`, `ConstantImpl.java`, `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/dsl/Param.java`.

**Constant** (`ConstantImpl<T>`, `@Immutable`): wraps a literal Java value known at query-construction time.
Small-integer/boxed-primitive values (`byte`, `char`, `short`, `int` 0-255, `long` 0-255, `boolean`) are served from static caches (`CACHE_SIZE = 256`) to avoid allocation churn; everything else calls `new ConstantImpl<>(type, value)`.
During serialization a `Constant` is turned into either an inline SQL literal (`useLiterals=true` path) or a positional bind placeholder plus an entry appended to the serializer's `constants` list (default, prepared-statement path) — see section 9.

**Param** (`ParamExpression`/`Param<T>`/`ParamExpressionImpl<T>`): a *named or anonymous placeholder* whose value is deliberately **not known** at query-construction time — it is bound later, once, right before execution, via `QueryMixin.set(ParamExpression<P>, P)` / `DefaultQueryMetadata.setParam` which stores into a `Map<ParamExpression<?>, Object> params` on the `QueryMetadata`.
- Named: `new Param<>(String.class, "name")` — `getNotSetMessage()` reports `"The parameter name needs to be set"`.
- Anonymous: `new Param<>(String.class)` — auto-generates a random 10-char hex name via `UUID.randomUUID()`, `getNotSetMessage()` reports `"A parameter of type ... was not set"`.

**When to use which**: use `Constant` (i.e. just pass a Java value into `.eq(value)`, which is auto-wrapped via `ExpressionUtils.toExpression`/`ConstantImpl.create`) for values known when the predicate tree is built.
Use `Param` for the specific case of a *reusable, precompiled query template* — build the `Expression`/`Predicate` tree once (e.g. as a static field or cached plan), and bind different concrete values into it on each execution via `.set(param, value)` without re-walking or re-serializing the expression tree.
This is the QueryDSL equivalent of a JDBC `PreparedStatement` reused across calls with different bind values, at the DSL level.

**Binding at execution time** (traced through `querydsl-libraries/querydsl-jpa/src/main/java/com/querydsl/jpa/impl/`):
1. The serializer (`SerializerBase.visit(ParamExpression<?>, Void)`, `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/support/SerializerBase.java`) does **not** resolve the param's value; it registers a placeholder label (`?<n>` for anonymous, `?<name>` else) and appends the `Param` object itself into the serializer's positional `constants` list, exactly like a real constant would be appended.
2. At execution time, `AbstractJPAQuery` calls `JPAUtil.setConstants(query, serializer.getConstants(), getMetadata().getParams())` (`querydsl-libraries/querydsl-jpa/src/main/java/com/querydsl/jpa/impl/JPAUtil.java`).
3. `JPAUtil.setConstants` iterates the positional constants list; for each entry that is actually a `Param` instance, it looks up the bound value in the `params` map (populated by `.set(...)`); **if absent, it throws `ParamNotSetException`**; otherwise it performs primitive-widening coercion and calls `query.setParameter(i+1, val)` — i.e. it is bound as a real positional JDBC/JPA bind parameter, never string-substituted into the query text.
4. `ParamsVisitor` (`querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/ParamsVisitor.java`) has a narrower job: when a `SubQueryExpression` is embedded in an outer query, it copies the *subquery's own* already-bound param values up into the outer `QueryMetadata`, so a subquery's params get bound too when the whole tree executes.

Net effect: `Param` is a genuinely parameterized bind variable end-to-end, not a templating mechanism — it is the safe complement to the raw-SQL escape hatches in section 9.

## 7. QueryFlag / JoinFlag

Files: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/QueryFlag.java`, `JoinFlag.java`, `JoinExpression.java`.

Both are explicit, documented **raw-SQL-injection points by design** — they exist specifically to let backend-specific query subclasses (e.g. `MySQLQuery`, `SQLServerTemplates`) inject dialect-specific syntax the core DSL doesn't model, at precise, named positions in the generated SQL/JPQL string.

`QueryFlag` wraps an `Expression<?>` flag (built from a plain string via `ExpressionUtils.template(Object.class, flag)` if constructed with a `String`) plus a `Position`:

| `QueryFlag.Position` | Insertion point |
|---|---|
| `WITH` | CTE `WITH` clause |
| `START` | Start of query |
| `START_OVERRIDE` | Replaces the first element (e.g. override `SELECT`/`INSERT` keyword) |
| `AFTER_SELECT` | Right after `SELECT` (e.g. MySQL `SELECT SQL_BIG_RESULT ...`) |
| `AFTER_PROJECTION` | After the select-list |
| `BEFORE_FILTERS` / `AFTER_FILTERS` | Around the `WHERE` clause |
| `BEFORE_GROUP_BY` / `AFTER_GROUP_BY` | Around `GROUP BY` |
| `BEFORE_HAVING` / `AFTER_HAVING` | Around `HAVING` |
| `BEFORE_ORDER` / `AFTER_ORDER` | Around `ORDER BY` |
| `END` | End of query |

`JoinFlag` wraps an `Expression<?>`/string flag plus a narrower `Position` enum scoped to a single join clause: `START`, `OVERRIDE` (replaces the join keyword itself), `BEFORE_TARGET`, `BEFORE_CONDITION`, `END`.
`JoinExpression` (one row of `QueryMetadata.getJoins()`) carries a `Set<JoinFlag>` alongside its type/target/condition.

Both are added via `QueryMetadata.addFlag(QueryFlag)` / `addJoinFlag(JoinFlag)`, typically exposed as fluent methods on a backend-specific query subclass, e.g. (from `docs/tutorials/sql.md`):

```java
public class MySQLQuery<T> extends AbstractSQLQuery<T, MySQLQuery<T>> {
    public MySQLQuery bigResult() {
        return addFlag(Position.AFTER_SELECT, "SQL_BIG_RESULT ");
    }
}
```

Real-world example of the injection risk this creates (`querydsl-libraries/querydsl-r2dbc/src/main/java/com/querydsl/r2dbc/mysql/AbstractR2DBCMySQLQuery.java`):

```java
return addJoinFlag(" force index (" + String.join(", ", indexes) + ")", JoinFlag.Position.END);
```

Index names are string-concatenated directly into the SQL text with **no escaping or allowlisting**.
If `indexes` ever derives from unsanitized user input, this is a direct SQL-injection point.
The library's own docs describe `QueryFlag`/`JoinFlag` payloads plainly as "custom SQL snippets that can be inserted at specific points in the serialization" — i.e. the mechanism is an intentional raw-string escape hatch, not a parameterized API, and callers are trusted to only feed it static, backend-author-controlled syntax (index hints, optimizer hints, CTE keywords), never end-user data.

## 8. OrderSpecifier

Files: `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/OrderSpecifier.java`, `Order.java`.

`OrderSpecifier<T extends Comparable>` is an immutable triple: `Order order` (`ASC`/`DESC`), `Expression<T> target`, `NullHandling nullHandling` (`Default`, `NullsFirst`, `NullsLast`).
`.nullsFirst()`/`.nullsLast()` return a *new* `OrderSpecifier` (immutable, non-mutating builder style) with `nullHandling` set; default construction leaves `NullHandling.Default` (dialect's native default ordering of nulls, unspecified by QueryDSL itself).

Rendering is backend/dialect-specific, not in `querydsl-core`:
- JPQL (`querydsl-libraries/querydsl-jpa/src/main/java/com/querydsl/jpa/JPQLSerializer.java`, ~line 260): emits literal `" nulls first"` / `" nulls last"` suffixes (JPQL 2.1+ / Hibernate HQL supports this natively).
- SQL (`querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/SQLSerializer.java`, ~line 439-474, mirrored in the R2DBC `SQLSerializer`): consults `SQLTemplates.getNullsFirst()`/`getNullsLast()` (dialect-configurable strings, default `" nulls first"`/`" nulls last"`).
  - Dialects with **no native support** (MySQL, DB2, SQLite, SQL Server, CUBRID, Teradata, Turso — each calls `setNullsFirst(null)`/`setNullsLast(null)` in their `*Templates` constructor) fall back to a synthesized ordering column: `(case when <target> is null then 0 else 1 end), <target> <asc|desc>` for nulls-first, and `0`/`1` swapped for nulls-last.
  This is a portable emulation technique worth replicating in a Rust reimplementation for dialects lacking `NULLS FIRST/LAST`.

## 9. Design recommendations for Rust

**BooleanBuilder equivalent.** Since Option-based combinators (e.g. `and_opt`, `filter_map` over `Option<Predicate>`, or a fold over `Vec<Option<Predicate>>`) already exist in idiomatic Rust, a *mutable stateful builder struct* mirroring `BooleanBuilder` is largely unnecessary as the primary API — the null-tolerance problem `BooleanBuilder` solves is exactly what `Option<Predicate>` combinators solve natively and more type-safely (no runtime null checks, no wrapper wrapping BooleanBuilder-in-BooleanBuilder).
Recommend:
- A free function or trait method, e.g. `and_all(impl IntoIterator<Item = Option<Predicate>>) -> Option<Predicate>` and `or_any(...)`, directly mirroring `ExpressionUtils.allOf`/`anyOf` — fold, skipping `None`, returning `None` if everything was `None`.
- `Predicate` (or an extension trait on `Option<Predicate>`) getting `.and(other: Option<Predicate>) -> Option<Predicate>` / `.or(...)` so call sites read as `where_clause = where_clause.and(name_filter).and(age_filter)`, chaining without a separate mutable builder type.
- Keep a small ergonomic builder struct *only* if callers need imperative-loop construction (`for x in dynamic_list { builder = builder.or(...) }`) — but make it an immutable-functional wrapper around `Option<Predicate>` with `and`/`or`/`not` consuming `self` and returning `Self`, not a mutable struct with interior nullable state.  This keeps `hasValue()`-equivalent as simply `builder.is_some()` on the `Option`.
- Do **not** replicate `andAnyOf`/`orAllOf` as bespoke methods if a generic `and_all`/`or_any` over an iterator already covers it — `andAnyOf(a, b, c)` is just `.and(or_any([a,b,c]))`.

**PathBuilder-style dynamic escape hatch.** A dynamic, string-keyed path escape hatch is worth including, because generated-schema/derive-macro-based static types (the Rust equivalent of Q-classes) cannot cover every use case: runtime-loaded schemas, generic admin/reporting tools, and schema introspected from a live database connection all need it.
To keep it type-honest (unlike QueryDSL's `PathBuilder.DEFAULT`, which is a security-only filter, not a correctness check):
- Make the *validated* path the only constructible path in the public API — i.e. there is no "DEFAULT: accept anything but whitespace" tier exposed by default.  Require callers to supply a schema/metadata source (a trait like `SchemaValidator: fn resolve(&self, parent: TypeId, property: &str) -> Option<ColumnType>`) at dynamic-path-builder construction time, mirroring `PathBuilderValidator::FIELDS`/`PROPERTIES`/the JPA-metamodel validator, not `PathBuilderValidator::DEFAULT`.
- Return a `Result<DynPath, UnknownColumnError>` from every `get`, forcing the caller to handle "property does not exist" at the point of dynamic access, rather than deferring to an opaque backend error at execution time as QueryDSL's `PathBuilder` does when only `DEFAULT` validation is active.
- Still allow an explicitly-named `unchecked` construction path (e.g. `DynPath::unchecked(name, type)`) for advanced users who really do have out-of-band knowledge the validator can't see (e.g. a raw SQL view column) — but name it so its lack of validation is obvious at the call site, unlike QueryDSL where `DEFAULT` looks safe but is not schema-validated.
- Preserve the whitespace/identifier-shape check QueryDSL added for CVE-2024-49203 as a baseline sanitizer *regardless* of validator tier, since it is cheap insurance against string-templated backends (anyone rendering identifiers via string formatting rather than a proper quoting function).

**Raw-SQL escape hatches (QueryFlag/JoinFlag/Expressions.template equivalents).** These are necessary — no DSL models every dialect's optimizer hints, CTEs, or vendor syntax — but QueryDSL's implementation (plain string concatenation into a `Position`-tagged fragment, e.g. the MySQL `force index` example in section 7) is a foot-gun if a caller ever feeds it end-user data.
Recommendations for the Rust equivalent:
- Model the escape hatch the same way as ordinary predicates: a raw-fragment expression node that holds a template string with `{0}`, `{1}`, ... placeholders *and a list of bound `Expression` arguments*, exactly like QueryDSL's own `TemplateExpression`/`Expressions::template` (section 2/3) already does it correctly.  Route flag/hint content through the same mechanism so *values* are always bound parameters, never string-concatenated — only structural syntax (keywords, position) is literal text.
- For genuinely value-free structural hints (index names, optimizer hint keywords) that must be identifiers rather than bind values (most SQL dialects cannot bind an identifier as a parameter), require them to pass through a dedicated `Identifier` newtype constructed via a strict allowlist regex (e.g. `^[A-Za-z_][A-Za-z0-9_]*$`) or the dialect's own quoting/escaping function, never accepted as a bare `&str` spliced into a template.  This directly closes the gap the MySQL `force index` example leaves open.
- Keep the flag/position taxonomy (`WITH`, `AFTER_SELECT`, `BEFORE_FILTERS`, etc., and the join-scoped `OVERRIDE`/`BEFORE_TARGET`/`BEFORE_CONDITION`) as an enum, mirroring `QueryFlag::Position`/`JoinFlag::Position` — the *position* taxonomy has no injection risk and is a legitimately useful, backend-agnostic abstraction worth keeping as-is.
- Params (section 6) already show the target design for value binding done right end-to-end: register a placeholder at build time, resolve to a real value only at execution time via a positional/named bind map, and hard-fail (`ParamNotSetException` equivalent) rather than silently binding null or an empty string if unset.  The Rust query-metadata/serializer boundary should preserve this same two-phase (build placeholders, then bind-and-execute) separation rather than ever formatting a bound value into SQL text as a string, even for the "use literals" debug-mode escape hatch QueryDSL's `AbstractSQLQuery.setUseLiterals(true)` provides (`querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/AbstractSQLQuery.java`) — if a literal-inlining debug mode is offered at all, route every value through the dialect's proper literal-escaping function (QueryDSL does this via `Configuration.asLiteral`/`Type.getLiteral`, `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/Configuration.java`), never raw `Display`/`to_string()`.

## Unresolved questions

- The task listed `support/DetachableSQLQuery` as a file to read; no such class exists in this fork (renamed/removed upstream).  The closest analogues found are `SubQueryExpression` (a `QueryMetadata`-holding value usable as a self-contained, connection-less subquery) and `AbstractSQLQuery.clone()`/`clone(Connection)` (`querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/AbstractSQLQuery.java`), which detach a query's metadata from a specific JDBC `Connection` for reuse.  Confirm with the planner whether this satisfies the "detached query support" research need or whether a different, older branch/version should be consulted.
- `PathType.TREATED_PATH` (JPA `TREAT` downcasts) was catalogued from the enum only; its serialization template lives in JPA-specific serializer code, not `querydsl-core/Templates.java`, and was not traced in depth since it is JPA-specific rather than core-DSL/Path-model behavior — flag if the planner needs JPA `TREAT` semantics specifically.

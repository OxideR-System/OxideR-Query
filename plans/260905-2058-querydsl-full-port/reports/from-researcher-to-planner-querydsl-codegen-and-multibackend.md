# QueryDSL Metamodel Generation and Multi-Backend Architecture

Research target: how the Q-type metamodel is generated (APT + SQL schema introspection), and the seam that lets one AST serve SQL, in-memory collections, and MongoDB.
All paths below are relative to the repo root `querydsl/`.
Note: the task brief pointed at `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/codegen/` for SQL codegen.
In this checkout that code actually lives at `querydsl-tooling/querydsl-sql-codegen/src/main/java/com/querydsl/sql/codegen/` (a tooling/library split happened at some point).
Likewise APT lives under `querydsl-tooling/querydsl-apt/` (there is also a near-empty legacy `querydsl-apt/` directory at repo root with a single stray file, not the real module).
`sql/types/` matches the brief exactly under `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/types/`.

## 1. Annotation catalog

All annotations live in `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/annotations/`.
The APT processor that reads them is `querydsl-tooling/querydsl-apt/src/main/java/com/querydsl/apt/QuerydslAnnotationProcessor.java`, which wires `QueryEntities`/`QueryEntity`/`QuerySupertype`/`QueryEmbeddable`/`QueryEmbedded`/`QueryTransient` into a `DefaultConfiguration`.

| Annotation | Target | Effect on codegen |
|---|---|---|
| `QueryEntity` | TYPE | Marks a domain class as a root entity. Processor generates a full `Q<Name>` type via `EntitySerializer` (`DefaultEntitySerializer`), with a static default instance. |
| `QuerySupertype` | TYPE | Marks a common base class. Generates a `Q<Name>` via `SupertypeSerializer` (`DefaultSupertypeSerializer`), no default instance; subclasses that extend it get a `_super` field wired to the same properties (see `EntityType.include`). |
| `QueryEmbeddable` | TYPE | Marks a class embeddable inside entities. Generates a `Q<Name>` via `EmbeddableSerializer` (`DefaultEmbeddableSerializer`, itself a subclass of `DefaultEntitySerializer` with a different class header - `BeanPath` and no static factories/default instance). |
| `QueryEmbedded` | FIELD, METHOD | Marks a property whose declared type should be pulled into codegen even without its own entity annotation. `AbstractQuerydslProcessor.getEmbeddedTypes()`/`collectElements()` walks fields/getters (and their `Collection`/`List`/`Set`/`Map` element types) annotated `@QueryEmbedded` and adds the referenced type as a processed element; if that type has no entity annotation it is classified as an embeddable (`context.embeddableTypes`). |
| `QuerySupertype`/`QueryEntity`/`QueryEmbeddable` combination via `QueryEntities` | PACKAGE | `QueryEntities` lets you list `Class<?>[]` on a package to force-generate Q-types for classes you cannot annotate directly (e.g. from another library). |
| `QueryExclude` | TYPE, PACKAGE | Opt-out: excludes an otherwise-matched type/package from generation (`AbstractQuerydslProcessor.processExclusions()`). |
| `QueryTransient` | FIELD, METHOD | Skips a field/getter during property collection (`DefaultConfiguration.isBlockedField/isBlockedGetter`). |
| `QueryType` | FIELD, METHOD, PARAMETER | Overrides the inferred `PropertyType` (`TypeElementHandler.toProperty`: `propertyType.as(TypeCategory.valueOf(...))`). `PropertyType.NONE` forces the property to be skipped entirely. |
| `PropertyType` (enum) | n/a | The value domain for `QueryType`: `COMPARABLE, ENUM, DATE, DATETIME, NONE, NUMERIC, SIMPLE, STRING, TIME, ENTITY`. Maps 1:1 onto a subset of `TypeCategory` names (validated via `TypeCategory.valueOf(name())`). |
| `QueryInit` | FIELD, METHOD | Declares eager-init paths for an entity-typed or collection-typed property, e.g. `@QueryInit("address.city")`. Stored as `Property.inits`; `DefaultEntitySerializer.introInits()` emits a `private static final PathInits INITS = new PathInits("*", "address.city")` field, and entity-typed properties are constructed eagerly in the constructor (`initEntityField`) instead of lazily on first accessor call. |
| `QueryProjection` | CONSTRUCTOR, TYPE | Marks a DTO constructor (or all constructors of a type) for a generated `Q<Dto>` `ConstructorExpression` type via `ProjectionSerializer`/`DefaultProjectionSerializer`. `useBuilder`/`builderName` additionally generate a fluent builder (`QDto.builderNew().field(...).build()`). |
| `QueryDelegate` | METHOD | Declares a static helper `(QEntity entity, ...extra) -> X` method that is woven into the generated `QEntity` as an instance method without the first argument (`DefaultEntitySerializer.delegate()`). |
| `Config` | TYPE, PACKAGE | Per-type/per-package override of `SerializerConfig` (entity accessors, list/map accessors, default-variable creation) via `DefaultConfiguration` reading `@Config`. |

Supertype/embedded property propagation: `AbstractQuerydslProcessor.addSupertypeFields()` recursively copies properties from `Supertype.getEntityType()` into the subtype (`EntityType.include()`), and marks copied `Property` instances `inherited=true`, which the serializer renders as `// inherited` and (when entity accessors are off) an alias assignment to `_super.field` rather than a fresh Path.

Collection/map properties are not a separate annotation - they are inferred structurally.
`ExtendedTypeFactory.createClassType()` recognizes anything assignable to `java.util.Map`/`List`/`Set`/`Collection` (erasure check against `java.*`) and produces `TypeCategory.MAP`/`LIST`/`SET`/`COLLECTION` with the generic argument(s) resolved recursively - so a `List<Address>` field becomes a `ListPath<Address, QAddress>` if `Address` is itself an entity/embeddable, purely from generics, no annotation needed.

## 2. Generated Q-type anatomy

No prebuilt Q-classes exist in this checkout (nothing under any `generated-sources`), so the shape below is reconstructed line-for-line from `querydsl-tooling/querydsl-codegen/src/main/java/com/querydsl/codegen/DefaultEntitySerializer.java`, which is the class that actually walks an `EntityType` and writes Java source with a `CodeWriter`.

For a domain class:

```java
@QueryEntity
class Person {
    String name;
    int age;
    @QueryInit("city")
    Address address;      // @QueryEmbeddable or @QueryEntity
    List<Phone> phones;    // Phone is itself an entity/embeddable
}
```

the generator produces (`QPerson.java`, package/imports/annotations elided):

```java
public class QPerson extends EntityPathBase<Person> {

    private static final long serialVersionUID = <hash>L;

    private static final PathInits INITS = new PathInits("*", "address.city");

    public static final QPerson person = new QPerson("person");

    public final StringPath name = createString("name");

    public final NumberPath<Integer> age = createNumber("age", Integer.class);

    public final QAddress address;

    public final ListPath<Phone, QPhone> phones = this.<Phone, QPhone>createList(
            "phones", Phone.class, QPhone.class, PathInits.DIRECT2);

    public QPerson(String variable) {
        this(Person.class, forVariable(variable), INITS);
    }

    public QPerson(Path<? extends Person> path) {
        this(path.getType(), path.getMetadata(), PathInits.getFor(path.getMetadata(), INITS));
    }

    public QPerson(PathMetadata metadata) {
        this(metadata, PathInits.getFor(metadata, INITS));
    }

    public QPerson(PathMetadata metadata, PathInits inits) {
        this(Person.class, metadata, inits);
    }

    public QPerson(Class<? extends Person> type, PathMetadata metadata, PathInits inits) {
        super(type, metadata, inits);
        this.address = inits.isInitialized("address")
                ? new QAddress(forProperty("address"), inits.get("address"))
                : null;
    }
}
```

Anatomy, mapped to the code that emits it (all in `DefaultEntitySerializer`):

- **Class header** (`introClassHeader`): extends `EntityPathBase<Person>` when the type has properties, or a scalar `Path` subtype (`ComparablePath`/`EnumPath`/`DatePath`/`DateTimePath`/`TimePath`/`NumberPath`/`StringPath`/`BooleanPath`) when it is a property-less alias for a simple Java type. Carries `@Generated("com.querydsl.codegen.DefaultEntitySerializer")` and a `serialVersionUID` derived from the model's full name hash.
- **`INITS` field** (`introInits`): a `PathInits` built from every property's `@QueryInit` value, only emitted when needed (has entity fields or explicit inits).
- **Static default instance** (`introDefaultInstance`): `public static final QPerson person = new QPerson("person")`, gated by `SerializerConfig.createDefaultVariable()`; the alias is the modified simple name (or an override), disambiguated against reserved keywords.
- **Property fields** (`serializeProperties`): one `public final` field per property, created via a family of `BeanPath` factory methods keyed by `TypeCategory` - `createString`, `createBoolean`, `createSimple`, `createComparable`, `createEnum`, `createDate`, `createDateTime`, `createTime`, `createNumber`, `createArray`, `createCollection`, `createSet`, `createList`, `createMap`; entity-typed fields instead get a `QXxx` field wired up via `entityField`/`initEntityField` and are conditionally null unless `inits.isInitialized(...)`. When `SerializerConfig.useEntityAccessors()` is on, entity fields are `protected` and exposed only via a lazily-initializing getter (`entityAccessor`); list/map accessors work the same way when those config flags are on.
- **Constructors** (`constructors`/`constructorsForVariables`): always a `(String variable)`, `(Path<? extends T> path)`, `(PathMetadata metadata)` constructor, plus `(PathMetadata, PathInits)` and `(Class<? extends T>, PathMetadata, PathInits)` overloads whenever the entity has entity-typed properties, because those need to propagate `PathInits` down the tree at construction time rather than at first access.
- **`_super` field** (`introSuper`): when a supertype's `EntityType` is known, a `public final QSuper _super` field is added, either eagerly `new QSuper(this)` (no entity fields upstream) or constructed in the entity constructor (`initEntityFields`) so that inherited entity-typed properties can alias `_super.field` instead of duplicating a Path.
- **Delegate methods** (`delegate`): `@QueryDelegate` static helpers become instance methods on the Q-type, forwarding `this` (or `this._super...` if declared on a supertype) as the first argument.
- **Factory methods for `@QueryProjection`** (`introFactoryMethods`): a `static <ConstructorExpression<Dto>> create(...)` per annotated constructor, built with `Projections.constructor(Dto.class, new Class<?>[]{...}, args...)`.

`DefaultEmbeddableSerializer` and `DefaultSupertypeSerializer` are thin subclasses of `DefaultEntitySerializer` (in the same package) that mostly change the class header (`BeanPath<T>` base, no static default instance for supertypes/embeddables) - the property/constructor logic above is shared.

## 3. TypeCategory / PropertyType classification

`TypeCategory` (`querydsl-tooling/querydsl-codegen-utils/src/main/java/com/querydsl/codegen/utils/model/TypeCategory.java`) is a flat enum with an optional supertype link (`isSubCategoryOf`) and a static `get(String className)` lookup keyed by hard-coded JDK class names.
`ExtendedTypeFactory` (APT) is what actually assigns a category to a Java type it encounters, then `JavaTypeMappings` (`querydsl-tooling/querydsl-codegen/src/main/java/com/querydsl/codegen/JavaTypeMappings.java`) maps each category to the Expression/Path/Template classes the serializer instantiates.

| TypeCategory | Matches (class-name based, `TypeCategory.get`) | superType | Expression class | Path class | Template class |
|---|---|---|---|---|---|
| STRING | `java.lang.String` | COMPARABLE | `StringExpression` | `StringPath` | `StringTemplate` |
| BOOLEAN | `java.lang.Boolean` | COMPARABLE | `BooleanExpression` | `BooleanPath` | `BooleanTemplate` |
| COMPARABLE | (fallback when type implements `Comparable`) | SIMPLE | `ComparableExpression` | `ComparablePath` | `ComparableTemplate` |
| ENUM | any `Enum` | COMPARABLE | `EnumExpression` | `EnumPath` | `EnumTemplate` |
| DATE | `java.sql.Date`, `java.time.LocalDate` | COMPARABLE | `TemporalExpression` | `DatePath` | `DateTemplate` |
| DATETIME | `Calendar`, `java.util.Date`, `java.sql.Timestamp`, `Instant`, `LocalDateTime`, `OffsetDateTime`, `ZonedDateTime` | COMPARABLE | `TemporalExpression` | `DateTimePath` | `DateTimeTemplate` |
| TIME | `java.sql.Time`, `LocalTime`, `OffsetTime` | COMPARABLE | `TemporalExpression` | `TimePath` | `TimeTemplate` |
| NUMERIC | (fallback when type is `Number` and `Comparable`, forced in `ExtendedTypeFactory.createClassType`) | COMPARABLE | `NumberExpression` | `NumberPath` | `NumberTemplate` |
| SIMPLE | default fallback for anything unmatched | (none) | `SimpleExpression` | `SimplePath` | `SimpleTemplate` |
| ARRAY | Java array types | (none) | `Expression` | `SimplePath`* | `SimpleTemplate` |
| COLLECTION | `java.util.Collection` | (none) | `Expression` | `SimplePath`* | `SimpleTemplate` |
| SET | `java.util.Set` | COLLECTION | `Expression` | `SimplePath`* | `SimpleTemplate` |
| LIST | `java.util.List` | COLLECTION | `Expression` | `SimplePath`* | `SimpleTemplate` |
| MAP | `java.util.Map` | (none) | `Expression` | `SimplePath`* | `SimpleTemplate` |
| CUSTOM | user-registered `Type` not otherwise categorized | (none) | `Expression` | `Path` | `SimpleTemplate` |
| ENTITY | annotated `@QueryEntity`/etc, or an `EntityType` instance | (none) | `Expression` | `Path` | `SimpleTemplate` |

\* `TypeMappings.getPathType` for ARRAY/COLLECTION/SET/LIST/MAP is overridden by `DefaultEntitySerializer.serializeProperties`, which hand-builds `ArrayPath`, `CollectionPath`, `SetPath`, `ListPath`, `MapPath` types directly (not the generic `SimplePath` mapping) - the `JavaTypeMappings` registration for those categories is really just a placeholder/fallback so `TypeMappings` never returns null.

`PropertyType` (the annotation enum) intentionally mirrors a subset of `TypeCategory` names (`COMPARABLE, ENUM, DATE, DATETIME, NONE, NUMERIC, SIMPLE, STRING, TIME, ENTITY`) so `@QueryType(PropertyType.X)` can do `TypeCategory.valueOf(x.name())` and call `Type.as(category)` to force-reclassify a property (`TypeElementHandler.toProperty`, `querydsl-tooling/querydsl-apt/src/main/java/com/querydsl/apt/TypeElementHandler.java`).
`PropertyType.NONE` is the escape hatch meaning "drop this property" - it has no `TypeCategory` counterpart.

Classification algorithm (`ExtendedTypeFactory.createClassType`, `querydsl-tooling/querydsl-apt/src/main/java/com/querydsl/apt/ExtendedTypeFactory.java`):
1. Map/List/Set/Collection assignability check first (only for `java.*` types).
2. `TypeCategory.get(className)` exact-match against the hard-coded JDK class table above.
3. If not already NUMERIC but the type is `Comparable` and a `Number` subtype, force NUMERIC.
4. Else if not already a COMPARABLE subcategory but the type is `Comparable`, force COMPARABLE.
5. If the type element (or its superclass) carries any of the configured entity annotations, force ENTITY and wrap it in an `EntityType`.

## 4. MetaDataExporter: JDBC introspection pipeline

Entry point: `querydsl-tooling/querydsl-sql-codegen/src/main/java/com/querydsl/sql/codegen/MetaDataExporter.java`, driven by a `MetadataExporterConfig` (interface `MetadataExporterConfig.java`, default impl `MetadataExporterConfigImpl.java`) and a DI-style `SQLCodegenModule` (extends `CodegenModule`).

Pipeline (`export(DatabaseMetaData md)`):
1. Bind config into the module: prefix/suffix, bean prefix/suffix, package name, `innerClassesForKeys`, `schemaToPackage`, imports, generated-annotation class, custom `NamingStrategy` class (default `DefaultNamingStrategy`).
2. Register custom SQL `Type`s, type-name mappings, numeric-precision mappings, and rename mappings onto the `com.querydsl.sql.Configuration` (`configureModule()`).
3. Detect the SQL dialect via `SQLTemplatesRegistry.getTemplates(md)` (matches on `DatabaseMetaData.getDatabaseProductName()`), and set it on the `Configuration` if found.
4. Build a `KeyDataFactory` bound to the naming strategy and package/prefix/suffix (targets bean or Q-type package depending on whether a bean serializer is configured).
5. For each (catalog x schema x table pattern) combination, call `handleTables` -> `DatabaseMetaData.getTables(catalog, schema, table, types)` where `types` is `{"TABLE","VIEW"}`, a caller-supplied comma list (`getTableTypesToExport`), or `null` for "export all table types".
6. Per table (`handleTable`):
   - Normalize catalog/schema/table names (`NamingStrategy.normalize*Name`), decide via `namingStrategy.shouldGenerateClass(schemaAndTable)` whether to skip it, then create an `EntityType` (`createEntityType`) tagged with `data["schema"]`/`data["table"]`.
   - Primary keys: `KeyDataFactory.getPrimaryKeys` -> `DatabaseMetaData.getPrimaryKeys`, stored as `PrimaryKeyData` on `classModel.getData()`.
   - Foreign keys (direct): `getImportedKeys` -> `DatabaseMetaData.getImportedKeys`, filtered through `namingStrategy.shouldGenerateForeignKey`.
   - Foreign keys (inverse): `getExportedKeys` -> `DatabaseMetaData.getExportedKeys`, giving other tables' FKs that point at this table.
   - Columns: `DatabaseMetaData.getColumns` iterated in `handleColumn`, which reads name/type/size/digits/nullable/default/ordinal-position (columns are read in ascending index order deliberately, to avoid an Oracle LOB-stream-closed bug when `COLUMN_DEF`, a streamed LONG, is read out of order relative to a later column), resolves the Java type via `Configuration.getJavaType(...)` (JDBC SQL type + type name + size/digits + table/column overrides), and creates a `Property` plus a `ColumnMetadata` (name, JDBC type, index, nullable, size, digits) stashed in `property.getData().put("COLUMN", ...)`.
   - Optional `@Column`/`@NotNull`/`@Size` bean-validation annotations are attached per `config.isColumnAnnotations()`/`isValidationAnnotations()`.
   - Indexes are not separately exported as first-class metadata in this pipeline - only PK/FK constraints feed the model (no `getIndexInfo()` call in `MetaDataExporter`).
   - Serialize the model (`serialize`) to either a single Q-type file, or a paired bean class (`BeanSerializer`) + Q-type wrapping it, written via `MetaDataSerializer` (extends `DefaultEntitySerializer`, adds schema/table-aware constructors, `addMetadata()` column registration, `PrimaryKeys`/`ForeignKeys` inner or flat fields).

`NamingStrategy` (`NamingStrategy.java`) is the single seam for table/column -> class/property name conversion, schema -> package mapping, and inclusion filtering; `AbstractNamingStrategy` supplies identifier-escaping defaults, `DefaultNamingStrategy` adds underscore-to-camelCase conversion (`toCamelCase`) and Java-keyword-safe suffixing (`Naming.normalize`, appends `Col`/`_col` etc.). Overridable per-method: class name, property name, PK/FK property names (with `Pk`/leading-underscore disambiguation), default alias, default variable name, package (schema folding), and two boolean gates (`shouldGenerateClass`, `shouldGenerateForeignKey`).

Options surface (`MetadataExporterConfig.java`, ~35 getters): name prefix/suffix, bean prefix/suffix, target/bean-target folders, Scala-source switch, package/bean-package name, inner-classes-for-keys, custom `NamingStrategy` class, catalog/schema/table name patterns (comma-separated lists supported), column/validation annotation switches, `schemaToPackage` (deprecated), lower-case normalization, export switches for tables/views/all-types/beans/PK/FK(direct)/FK(inverse), source encoding, extra table-types-to-export string, extra imports, generated-annotation class override, custom bean serializer class + interfaces + toString/full-constructor/print-supertype switches, custom `Type` registrations, type/numeric/rename mappings, column comparator class, and a fully custom `Serializer` class override.

## 5. sql/types: Java type <-> JDBC type registry

`querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/types/Type.java` is the core contract:

```java
public interface Type<T> {
  int[] getSQLTypes();                 // java.sql.Types codes this Type handles
  Class<T> getReturnedClass();
  String getLiteral(T value);
  T getValue(ResultSet rs, int startIndex) throws SQLException;
  default T getValue(ResultSet rs, int startIndex, Class<T> clazz) { ... }
  void setValue(PreparedStatement st, int startIndex, T value) throws SQLException;
}
```

`AbstractType<T>` fixes a single `java.sql.Types` constant and a default `toString()`-based literal.
Concrete implementations in the same package cover the JDK's boxed/date/time/binary types (`BigDecimalType`, `BigIntegerType`, `BooleanType`, `ByteType`, `BytesType`, `CalendarType`, `CharacterType`, date/time family split between legacy `DateType`/`TimeType`/`TimestampType`/`UtilDateType` and JSR-310 `InstantType`/`LocalDateType`/`LocalDateTimeType`/`LocalTimeType`/`OffsetDateTimeType`/`OffsetTimeType`/`ZonedDateTimeType`, `EnumAsObjectType`/`EnumByNameType`/`EnumByOrdinalType` for enums, `BlobType`/`ClobType`/`InputStreamType`, `ArrayType` for JDBC arrays, `UtilUUIDType`, `LocaleType`, `CurrencyType`, `URLType`, plus dialect-shim types like `NumericBooleanType`/`TrueFalseType`/`YesNoType` for booleans stored as numbers/strings).

The registry lives in two collaborating classes inside `com.querydsl.sql` (not `sql.types`):
- `JavaTypeMapping` (package-private, `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/JavaTypeMapping.java`): a static `defaultTypes: Map<Class<?>, Type<?>>` seeded once with ~28 default types (registered for both boxed and unboxed primitive class where applicable), plus an instance-level `typeByClass` (user overrides, walked up the class hierarchy and then interfaces on lookup miss) and `typeByColumn: Map<table, Map<column, Type>>` for table/column-specific overrides.
- `JDBCTypeMapping`: the inverse direction, `java.sql.Types` code (+size/digits for NUMERIC) -> Java `Class`.

`com.querydsl.sql.Configuration` (`querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/Configuration.java`) is the facade applications and `MetaDataExporter` use:
- `register(Type<?> type)` - registers a custom `Type` both ways: `jdbcTypeMapping.register(sqlTypeCode, javaClass)` and `javaTypeMapping.register(type)`.
- `register(String table, String column, Type<?> | Class<?> javaType)` - table/column-scoped override, consulted first in `getJavaType(...)`.
- `registerType(String typeName, Class<?> clazz)` - JDBC type-name string (e.g. a vendor-specific `TYPE_NAME`) to Java class, consulted before the generic JDBC-code fallback.
- `registerNumeric(total, decimal, javaType)` / ranged overload - overrides the Java type chosen for `NUMERIC(total,decimal)` columns (e.g. force `NUMERIC(19,2)` to `BigDecimal` vs a narrower type).
- `getJavaType(sqlType, typeName, size, digits, tableName, columnName)` - the resolution order `MetaDataExporter.handleColumn` actually calls: table.column override -> type-name-string override (with array-type unwrapping for `_foo`/`foo array`/`foo[]`/`foo(n)` conventions) -> JDBC-code+size+digits fallback via `JDBCTypeMapping`.
- `asLiteral(Object)` / `getType(Class)` - used by `SQLSerializer` for inline literal rendering, and by `MetaDataExporter.handleColumn` (`asModel`) when a registered `Type` is a `com.querydsl.sql.types.SimpleType` marker used to force a specific Q-type Path class for a column.
- `getType(table, column)` - raw lookup of a table/column override `Type`, used to detect custom Path classes during export.

Net effect for the Rust port: this is a two-way, override-able registry keyed by (Java class -> `Type`) and (JDBC type code/name -> Java class), with three levels of override granularity (global default, type-name string, table.column) plus special-cased NUMERIC precision/scale routing.

## 6. The multi-backend seam

The seam is the classic Visitor pattern rooted in two interfaces in `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/types/`:

```java
public interface Expression<T> extends Serializable {
  <R, C> R accept(Visitor<R, C> v, C context);
  Class<? extends T> getType();
}

public interface Visitor<R, C> {
  R visit(Constant<?> expr, C context);
  R visit(FactoryExpression<?> expr, C context);
  R visit(Operation<?> expr, C context);
  R visit(ParamExpression<?> expr, C context);
  R visit(Path<?> expr, C context);
  R visit(SubQueryExpression<?> expr, C context);
  R visit(TemplateExpression<?> expr, C context);
}
```

Every AST node QueryDSL ever builds - `Path` (property/variable references, including the Q-type fields), `Operation` (binary/n-ary operators and function calls, keyed by an `Operator` such as `Ops.EQ`/`Ops.AND`/`Ops.LIKE`), `Constant`, `TemplateExpression` (custom syntax escape hatch), `FactoryExpression` (row-to-object projections, e.g. `@QueryProjection` DTOs), `ParamExpression`, `SubQueryExpression` - is one of these seven closed cases.
`accept()` just does `v.visit(this, context)`; there is no backend-specific logic anywhere in the AST classes themselves.
A backend is *only* an implementation of `Visitor<R, C>` that decides what "visiting" means, with its own `R` (return type) and `C` (context type).

Three concrete strategies observed for `R`:

1. **Text accumulation (SQL, JPQL, in-memory Java source).** `querydsl-libraries/querydsl-core/src/main/java/com/querydsl/core/support/SerializerBase.java` is `abstract class SerializerBase<S extends SerializerBase<S>> implements Visitor<Void, Void>`: it owns a `StringBuilder`, a constant/param table, and an `Ops -> Template` lookup (`Templates`), and each `visit(...)` override appends fragments to the buffer using dialect-specific templates. Three unrelated backends extend it with only their `Templates` differing:
   - `querydsl-libraries/querydsl-sql/src/main/java/com/querydsl/sql/SQLSerializer.java` (`SQLTemplates` -> ANSI/vendor SQL text).
   - `querydsl-libraries/querydsl-jpa/src/main/java/com/querydsl/jpa/JPQLSerializer.java` (JPQL/HQL text).
   - `querydsl-libraries/querydsl-collections/src/main/java/com/querydsl/collections/CollQuerySerializer.java` (`CollQueryTemplates` -> **Java source code text**, e.g. renders `Ops.EQ` as `" == "`).
2. **Compile-and-run (in-memory collections).** The Java source text `CollQuerySerializer` produces is not interpreted - `querydsl-libraries/querydsl-collections/src/main/java/com/querydsl/collections/DefaultEvaluatorFactory.java` hands it to an `EvaluatorFactory` (`querydsl-tooling/querydsl-codegen-utils/.../EvaluatorFactory.java`, implemented by `JDKEvaluatorFactory` using `javax.tools.JavaCompiler` or `ECJEvaluatorFactory` using the Eclipse compiler for OSGi/JRE-without-tools.jar environments), which compiles it into a real `.class` at runtime and returns an `Evaluator<T>` (`evaluate(Object... args)`), effectively JIT-compiling each distinct predicate/projection into bytecode. `DefaultQueryEngine` (implements `QueryEngine`) drives this per query against `Map<Expression<?>, Iterable<?>>` sources for count/exists/list.
3. **Direct object-tree construction (MongoDB).** `querydsl-libraries/querydsl-mongodb/src/main/java/com/querydsl/mongodb/MongodbSerializer.java` is `abstract class MongodbSerializer implements Visitor<Object, Void>` - no string buffer at all. Each `visit(...)` returns a `BasicDBObject`/`BasicDBList`/scalar directly, e.g. an `Operation` for `Ops.EQ` becomes `new BasicDBObject(key, value)` and `Ops.AND` becomes `{"$and": [...]}`. The visitor's return value *is* the Mongo query document.

So the contract a new backend must satisfy is exactly: implement `Visitor<R, C>` for the seven expression kinds, decide what `R` means for that backend (SQL string, compiled evaluator, BSON document, or something else entirely), and drive it by calling `expression.accept(this, context)` at the query root.
Nothing about `Path`, `Operation`, `EntityType`, or the generated Q-types needs to change to add a backend; only a new `Visitor` implementation (optionally plus a metadata-model translation layer analogous to `RelationalPathBase` for SQL) is required.
`SQLSerializer`, `JPQLSerializer`, and `CollQuerySerializer` additionally share `SerializerBase` because they all happen to target "generate text," but that base class is a convenience, not part of the contract - `MongodbSerializer` proves a backend can ignore it entirely.

## 7. Recommendations for the Rust port

**Derive macro surface.** To match the annotation catalog in section 1, a `#[derive(Entity)]`-style macro (or a small family: `Entity`, `Embeddable`, `Supertype`, `Projection`) needs to accept, at minimum:
- Per-field: an escape hatch equivalent to `@QueryTransient` (skip), `@QueryType` (force a property-kind override, e.g. treat a `newtype` wrapper as NUMERIC), and `@QueryInit` (eager-init path list for nested entity/collection fields - in Rust this matters more since there is no runtime "lazy field," so this maps to which nested Q-struct fields get pre-populated vs `None`/lazily built).
- Struct-level: entity vs. supertype vs. embeddable classification (could be three derive macros or one with an attribute), plus an equivalent to `QueryProjection`'s `useBuilder`/`builderName` for typed DTO projections.
- A way to declare "delegate" free functions attached to a generated struct (`QueryDelegate` equivalent), since Rust has no instance-method-injection trick available to a derive macro the way Java's codegen can just emit a wrapper method.
- Struct inheritance/composition: Rust has no class inheritance, so the `QuerySupertype`/`_super` pattern should become explicit composition (`#[query(flatten)] base: BaseFields`) with the macro copying/promoting the base struct's generated Path fields onto the child's Q-struct, mirroring `EntityType.include()`'s copy-with-inherited-flag semantics rather than trying to fake Java's `_super` delegation field.
- Collections/maps should be inferred structurally from `Vec<T>`/`HashMap<K,V>`/`Option<T>` the same way QueryDSL infers from `List<T>`/generics - no separate annotation needed, matching section 1's finding that this was never annotation-driven in Java either.

**Schema introspection for Postgres/MySQL/SQLite.** Mirror `MetaDataExporter`'s coverage, adjusted for what each driver actually exposes:
- Tables and views (with an explicit include/exclude type list, not just "all").
- Columns: name, ordinal position, nullability, declared size/precision/scale, default value presence (used to decide if a NOT NULL column still needs an `Option` in Rust when it has a server-side default), and both an "engine type name" (e.g. Postgres `numeric`, `jsonb`, `uuid`) and a generic SQL type code, because the exporter's real type-resolution order (table.column override -> type-name string override -> generic code+size/digits fallback) is what let QueryDSL support both "sane genericized types" and "vendor quirks" without special-casing every driver - worth preserving as three override tiers in the Rust registry.
- Primary keys and both directions of foreign keys (direct: this table's FKs; inverse: who references this table) - QueryDSL treats these as first-class model data attached to the entity, not just a Rust-side comment; that is what backs generated join helpers.
- Indexes (uniqueness, column order) - notably, section 4 shows QueryDSL's own `MetaDataExporter` does **not** actually export index metadata (`getIndexInfo()` is never called) even though PK/FK are exported; the Rust port should not copy that gap if migration-adjacent tooling or uniqueness-aware query building is a goal.
- Postgres-specific: array types, enum types (`pg_enum`), domains/composite types if in scope.
- MySQL-specific: `AUTO_INCREMENT`, unsigned integer variants (no unsigned types in the JDBC/SQL-standard model QueryDSL followed, but relevant to a Rust numeric mapping).
- SQLite-specific: its type affinity system (TEXT/NUMERIC/INTEGER/REAL/BLOB, not fixed column types) requires a fundamentally looser type-resolution fallback than the size/digits/type-name approach in `Configuration.getJavaType`, since SQLite drivers often report affinities rather than the declared type.
- A `NamingStrategy`-equivalent trait, cleanly separated (as in `NamingStrategy.java`) from the introspection code, covering: table/column -> struct/field identifier conversion, schema -> module/namespace mapping, PK/FK property naming (with the collision-avoidance behavior `DefaultNamingStrategy` shows, e.g. suffixing `Pk`), and per-table/per-FK inclusion predicates.

**Structuring the AST so a non-SQL backend can be added later without forking it.** The single most important structural lesson from section 6: keep the expression tree backend-agnostic by construction, not by convention.
Concretely:
- Define one closed `Expression` enum (or a small sealed trait hierarchy) covering the same seven cases QueryDSL uses - constant, path, operation (with an `Operator` enum, not a string), factory/projection expression, param placeholder, subquery, template/raw-escape-hatch - and never let a backend crate add a new AST variant. New capability should always be expressible as a new `Operator` value plus backend-specific interpretation of it, exactly like QueryDSL's `Ops` enum plus per-`Templates` string rendering.
- Define one visitor-shaped trait (`trait ExprVisitor<R> { fn visit_op(&mut self, op: &Operation) -> R; ... }` or a `match`-based free function if Rust's lack of double dispatch makes a literal visitor awkward) that every backend crate implements independently. A SQL backend's `R` is `(String, Vec<Value>)` (text + bound params); an in-memory backend's `R` is a boxed closure/compiled predicate (Rust has no runtime `javac`, so "compile the tree" should mean "fold the tree into a `Fn(&Row) -> bool` closure directly," skipping QueryDSL's detour through Java-source-text-then-javac entirely - that detour was a Java-specific workaround, not something worth preserving); a document-store backend's `R` is a JSON/BSON-like value tree, directly mirroring `MongodbSerializer`.
- Keep the generated Q-struct's fields backend-agnostic `Path` markers (roughly: table/column metadata + Rust type), and put all backend-specific rendering (SQL dialect quoting, Mongo operator names, in-memory closure construction) in per-backend crates that only ever consume the shared `Expression`/`Operator` types - never in the derive macro output itself. This is exactly what keeps `EntityType`/`Property`/the generated Q-type unchanged across `querydsl-sql`, `querydsl-collections`, and `querydsl-mongodb` in the Java codebase.
- Treat "operator support" as a per-backend capability table (QueryDSL's `Templates.getTemplate(Operator)` returning null/throwing for unsupported ops is the equivalent), so a backend can reject at build- or query-build-time an operator it cannot express (e.g. a key-value store rejecting `LIKE`) without the shared AST needing to know which backends exist.

## Unresolved questions

- The task brief's listed SQL codegen path (`querydsl-libraries/querydsl-sql/.../codegen/`) does not match this checkout's actual layout (`querydsl-tooling/querydsl-sql-codegen/`); worth confirming with whoever wrote the brief whether they were looking at an older upstream layout, in case other assumptions carried over from that source also need re-verified against this fork.
- No generated Q-type sources exist anywhere in the checked-out tree (nothing built yet), so section 2's example is a reconstruction from the serializer, not a verified compiler-checked artifact; recommend running `./mvnw -Pquickbuild,sql clean install` (or the APT test module) once to diff a real generated file against this reconstruction before it drives Rust macro design.
- `MetaDataExporter` does not export index metadata at all (confirmed by absence of any `getIndexInfo()` call) - flagged in section 7, but worth an explicit decision on whether the Rust port intentionally fixes this gap or intentionally matches it for behavior parity.

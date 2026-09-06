//! The query metamodel: entities and their typed columns.
//!
//! `#[derive(Entity)]` implements [`Entity`] for a struct and generates one
//! associated-const [`Column`] per field, so queries read as `User::id`,
//! mirroring QueryDSL's `QUser.user.id` without a separate metamodel type to
//! name.
//!
//! A [`Column`] carries two compile-time markers: the owning entity `E`, which
//! drives the "did you join that table" check, and the Rust type `T`, which
//! gates operators and drives value binding.

use crate::ast::node::{ColumnRef, Node, TableRef};
use crate::source::{Cons, Nil};
use crate::typed::expr::IntoExpr;
use core::marker::PhantomData;

/// A queryable database entity.
///
/// Implemented by `#[derive(Entity)]`.
pub trait Entity: Sized {
    /// The table name as it appears in SQL.
    const TABLE: &'static str;

    /// The schema qualifier, when the table is not in the default schema.
    const SCHEMA: Option<&'static str> = None;

    /// The unaliased reference to this entity's table.
    fn table_ref() -> TableRef {
        TableRef {
            schema: Self::SCHEMA,
            name: Self::TABLE,
            alias: None,
        }
    }

    /// The reference to this entity's table under an alias.
    fn aliased(alias: &'static str) -> TableRef {
        TableRef {
            schema: Self::SCHEMA,
            name: Self::TABLE,
            alias: Some(alias),
        }
    }

    /// This entity's table as a join target.
    fn table() -> Table<Self> {
        Table::new()
    }

    /// Begin a SELECT over this entity.
    fn query() -> crate::builder::Select<Cons<Self, Nil>> {
        crate::builder::Select::new(Self::table_ref())
    }

    /// Begin a SELECT over this entity under an alias.
    ///
    /// The scope holds [`Aliased<Self>`], matching the columns written
    /// `User::id.at(alias)`.
    fn query_as(alias: &'static str) -> crate::builder::Select<Cons<Aliased<Self>, Nil>> {
        crate::builder::Select::new(Self::aliased(alias))
    }

    /// Begin an INSERT into this entity's table.
    fn insert() -> crate::builder::Insert<Self> {
        crate::builder::Insert::new(Self::table_ref())
    }

    /// Begin an UPDATE of this entity's table.
    fn update() -> crate::builder::Update<Self> {
        crate::builder::Update::new(Self::table_ref())
    }

    /// Begin a DELETE from this entity's table.
    fn delete() -> crate::builder::Delete<Self> {
        crate::builder::Delete::new(Self::table_ref())
    }
}

/// A typed table column: owning entity `E`, Rust type `T`.
///
/// `nullable` is tracked at runtime rather than in the type. Type-level
/// nullability would have to widen every column of an outer-joined table, which
/// only becomes observable once rows are mapped into structs; until there is a
/// consumer for it, the flag is what the codegen and documentation need and the
/// type machinery would be cost without benefit.
pub struct Column<E, T> {
    /// The table this column belongs to, including any alias.
    pub table: TableRef,
    /// The column name.
    pub name: &'static str,
    /// Whether the column is nullable, derived from `Option<T>` fields.
    pub nullable: bool,
    // `fn() -> (E, T)` keeps `Column` `Copy`/`Send`/`Sync` without bounding E/T.
    _marker: PhantomData<fn() -> (E, T)>,
}

impl<E, T> Column<E, T> {
    /// Construct a column. Called by generated metamodel code.
    pub const fn new(table: &'static str, name: &'static str, nullable: bool) -> Self {
        Column {
            table: TableRef::new(table),
            name,
            nullable,
            _marker: PhantomData,
        }
    }

    /// Construct a column in a named schema.
    pub const fn in_schema(
        schema: &'static str,
        table: &'static str,
        name: &'static str,
        nullable: bool,
    ) -> Self {
        Column {
            table: TableRef {
                schema: Some(schema),
                name: table,
                alias: None,
            },
            name,
            nullable,
            _marker: PhantomData,
        }
    }

    /// The same column, qualified by a table alias.
    ///
    /// This is what makes self-joins expressible: `User::id.at("m")` refers to
    /// the `m` copy of the users table rather than the default one. The entity
    /// marker changes to [`Aliased<E>`], so the two copies are different
    /// entities as far as the scope check is concerned and a reference to
    /// either one is unambiguous.
    pub const fn at(self, alias: &'static str) -> Column<Aliased<E>, T> {
        Column {
            table: TableRef {
                schema: self.table.schema,
                name: self.table.name,
                alias: Some(alias),
            },
            name: self.name,
            nullable: self.nullable,
            _marker: PhantomData,
        }
    }

    /// The AST reference for this column.
    pub fn column_ref(&self) -> ColumnRef {
        ColumnRef {
            table: self.table,
            name: self.name,
        }
    }
}

// Manual Clone/Copy: the markers are zero-sized and never instantiated, so the
// derived bounds on E and T would be wrong.
impl<E, T> Clone for Column<E, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E, T> Copy for Column<E, T> {}

impl<E, T> core::fmt::Debug for Column<E, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Column({}.{})", self.table.qualifier(), self.name)
    }
}

impl<E, T> IntoExpr<T> for Column<E, T> {
    type Sources = Cons<E, Nil>;
    fn into_expr_node(self) -> Node {
        Node::Column(self.column_ref())
    }
}

/// The type-level identity of a table brought into a query under an alias.
///
/// A self-join puts the same table in scope twice. Without a second identity
/// the membership proof would be ambiguous - the compiler could satisfy
/// "`User` is in scope" from either copy - and the query would not compile at
/// all. Wrapping the aliased copy keeps every reference unambiguous, so
/// `User::id` and `User::id.at("manager")` are different columns to the
/// compiler as well as to the database.
///
/// One alias per entity per query is what this supports. A query needing a
/// third copy of one table reaches for [`col`](crate::typed::col), which names
/// the qualifier directly and forgoes the scope check.
pub struct Aliased<E>(PhantomData<fn() -> E>);

impl<E: Entity> Entity for Aliased<E> {
    const TABLE: &'static str = E::TABLE;
    const SCHEMA: Option<&'static str> = E::SCHEMA;
}

/// A table in a query, tagged with the entity it holds.
///
/// Joins take one of these rather than a bare type parameter, so
/// `.inner_join(Post::table(), ...)` infers the joined entity from the argument
/// instead of needing a turbofish. Aliasing it is what makes a self-join
/// expressible.
pub struct Table<E> {
    reference: TableRef,
    _marker: PhantomData<fn() -> E>,
}

impl<E: Entity> Default for Table<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity> Table<E> {
    /// The entity's table, unaliased.
    pub fn new() -> Self {
        Table {
            reference: E::table_ref(),
            _marker: PhantomData,
        }
    }
}

impl<E> Table<E> {
    /// The same table under an alias.
    ///
    /// Columns of the aliased copy are written `User::id.at("m")`, matching the
    /// alias given here, and carry the matching [`Aliased<E>`] marker.
    pub fn alias(self, alias: &'static str) -> Table<Aliased<E>> {
        Table {
            reference: TableRef {
                schema: self.reference.schema,
                name: self.reference.name,
                alias: Some(alias),
            },
            _marker: PhantomData,
        }
    }

    /// The underlying AST reference.
    pub fn reference(&self) -> TableRef {
        self.reference
    }
}

impl<E> Clone for Table<E> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E> Copy for Table<E> {}

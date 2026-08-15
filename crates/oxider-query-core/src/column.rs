//! The query metamodel: entities and their typed columns.
//!
//! `#[derive(Entity)]` implements [`Entity`] for a struct and generates one
//! associated-const [`Column`] per field, so queries read as `User::id`,
//! `User::name`, mirroring QueryDSL's `QUser.user.id` but without a separate
//! metamodel type.
//!
//! A [`Column`] carries two compile-time markers: the owning entity `E` and the
//! Rust type `T` of the column. `E` drives type-level source tracking (join keys
//! and "did you join this table?" checks); `T` gates operators and drives value
//! binding. Comparisons only accept a value convertible into `T`, so type
//! mismatches fail to compile.

use crate::expr::{BinOp, Expr};
use crate::predicate::{Order, OrderDir, Predicate};
use crate::source::{Cons, Nil};
use crate::value::{Orderable, ToSqlValue};
use core::marker::PhantomData;

/// A queryable database entity.
///
/// Implemented by `#[derive(Entity)]`. Start a query with `MyEntity::query()`.
pub trait Entity: Sized {
    /// The table name as it appears in SQL.
    const TABLE: &'static str;

    /// Begin a SELECT query over this entity.
    fn query() -> crate::query::Select<Cons<Self, Nil>> {
        crate::query::Select::new(Self::TABLE)
    }
}

/// The type-level source set contributed by a single column of entity `E`.
type Only<E> = Cons<E, Nil>;

/// A typed table column: owning entity `E` and Rust type `T`.
///
/// `nullable` is tracked at runtime for now; type-level nullability (so outer
/// joins widen non-null columns) is planned for a later phase.
pub struct Column<E, T> {
    /// Owning table name.
    pub table: &'static str,
    /// Column name.
    pub name: &'static str,
    /// Whether the column is nullable (derived from `Option<T>` fields).
    pub nullable: bool,
    // `fn() -> (E, T)` keeps `Column` `Copy`/`Send`/`Sync` without bounding E/T.
    _marker: PhantomData<fn() -> (E, T)>,
}

impl<E, T> Column<E, T> {
    /// Construct a column. Called by generated metamodel code.
    pub const fn new(table: &'static str, name: &'static str, nullable: bool) -> Self {
        Column {
            table,
            name,
            nullable,
            _marker: PhantomData,
        }
    }

    /// Lower this column to an AST reference.
    fn column_expr(&self) -> Expr {
        Expr::Column {
            table: self.table,
            name: self.name,
        }
    }
}

// Manual Clone/Copy: the markers are zero-sized and never instantiated.
impl<E, T> Clone for Column<E, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<E, T> Copy for Column<E, T> {}

/// Equality operators, available on every column whose type binds a value.
impl<E, T: ToSqlValue> Column<E, T> {
    /// `self = value`
    pub fn eq<V: Into<T>>(self, value: V) -> Predicate<Only<E>> {
        self.compare(BinOp::Eq, value)
    }

    /// `self <> value`
    pub fn ne<V: Into<T>>(self, value: V) -> Predicate<Only<E>> {
        self.compare(BinOp::Ne, value)
    }

    /// Compare against another column of the same type, e.g. a join key.
    ///
    /// Requires both columns to share the Rust type `T`, so mismatched keys are
    /// a compile error. The result references both entities.
    pub fn eq_column<E2>(self, other: Column<E2, T>) -> Predicate<Cons<E, Cons<E2, Nil>>> {
        Predicate::new(Expr::Binary {
            op: BinOp::Eq,
            lhs: Box::new(self.column_expr()),
            rhs: Box::new(other.column_expr()),
        })
    }

    fn compare<V: Into<T>>(self, op: BinOp, value: V) -> Predicate<Only<E>> {
        let bound = value.into().to_sql_value();
        Predicate::new(Expr::Binary {
            op,
            lhs: Box::new(self.column_expr()),
            rhs: Box::new(Expr::Param(bound)),
        })
    }
}

/// Ordering operators, available only on orderable column types.
impl<E, T: ToSqlValue + Orderable> Column<E, T> {
    /// `self > value`
    pub fn gt<V: Into<T>>(self, value: V) -> Predicate<Only<E>> {
        self.compare(BinOp::Gt, value)
    }
    /// `self >= value`
    pub fn ge<V: Into<T>>(self, value: V) -> Predicate<Only<E>> {
        self.compare(BinOp::Ge, value)
    }
    /// `self < value`
    pub fn lt<V: Into<T>>(self, value: V) -> Predicate<Only<E>> {
        self.compare(BinOp::Lt, value)
    }
    /// `self <= value`
    pub fn le<V: Into<T>>(self, value: V) -> Predicate<Only<E>> {
        self.compare(BinOp::Le, value)
    }

    /// Order by this column ascending.
    pub fn asc(self) -> Order<Only<E>> {
        Order::new(self.column_expr(), OrderDir::Asc)
    }
    /// Order by this column descending.
    pub fn desc(self) -> Order<Only<E>> {
        Order::new(self.column_expr(), OrderDir::Desc)
    }
}

/// Text operators, available only on `String` columns.
impl<E> Column<E, String> {
    /// `self LIKE pattern` (pattern used verbatim).
    pub fn like(self, pattern: impl Into<String>) -> Predicate<Only<E>> {
        self.like_pattern(pattern.into())
    }

    /// `self LIKE '%substring%'`.
    ///
    /// Note: LIKE metacharacters (`%`, `_`) in `substring` are not escaped yet;
    /// escaping with an ESCAPE clause is a planned text-ops hardening step.
    pub fn contains(self, substring: impl Into<String>) -> Predicate<Only<E>> {
        self.like_pattern(format!("%{}%", substring.into()))
    }

    /// `self LIKE 'prefix%'`.
    pub fn starts_with(self, prefix: impl Into<String>) -> Predicate<Only<E>> {
        self.like_pattern(format!("{}%", prefix.into()))
    }

    fn like_pattern(self, pattern: String) -> Predicate<Only<E>> {
        Predicate::new(Expr::Binary {
            op: BinOp::Like,
            lhs: Box::new(self.column_expr()),
            rhs: Box::new(Expr::Param(crate::value::Value::Text(pattern))),
        })
    }
}

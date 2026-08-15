//! INSERT, UPDATE, and DELETE statement builders.
//!
//! Single-table by design (no joins), so the in-scope set is always just the
//! target entity `E`. `set`/`value` take a typed [`Column`] of `E` and a value
//! convertible into the column's type, keeping column ownership and value types
//! checked. WHERE predicates are source-checked against `E` exactly like SELECT.

use crate::column::Column;
use crate::dialect::Dialect;
use crate::expr::{BinOp, Expr};
use crate::predicate::Predicate;
use crate::render::{render_expr, Rendered};
use crate::source::{Cons, ContainsAll, Nil};
use crate::value::{ToSqlValue, Value};
use core::fmt::Write;
use core::marker::PhantomData;

/// The single-entity in-scope set for a mutation targeting `E`.
type Scope<E> = Cons<E, Nil>;

/// An assigned column/value pair for INSERT and UPDATE.
struct Assignment {
    column: &'static str,
    value: Value,
}

/// `INSERT INTO table (...) VALUES (...)` builder for entity `E`.
pub struct Insert<E> {
    table: &'static str,
    assignments: Vec<Assignment>,
    _marker: PhantomData<fn() -> E>,
}

impl<E> Insert<E> {
    /// Start an INSERT into `table`. Called by [`Entity::insert`](crate::Entity).
    pub(crate) fn new(table: &'static str) -> Self {
        Insert {
            table,
            assignments: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Assign `column = value` for the inserted row. Repeat per column.
    pub fn value<T: ToSqlValue, V: Into<T>>(mut self, column: Column<E, T>, value: V) -> Self {
        self.assignments.push(Assignment {
            column: column.name,
            value: value.into().to_sql_value(),
        });
        self
    }

    /// Render for a specific dialect.
    pub fn render<D: Dialect>(&self, dialect: &D) -> Rendered {
        let mut params = Vec::with_capacity(self.assignments.len());
        let mut sql = String::from("INSERT INTO ");
        sql.push_str(&dialect.quote_ident(self.table));

        let cols: Vec<String> = self
            .assignments
            .iter()
            .map(|a| dialect.quote_ident(a.column))
            .collect();
        let placeholders: Vec<String> = self
            .assignments
            .iter()
            .map(|a| {
                params.push(a.value.clone());
                dialect.placeholder(params.len())
            })
            .collect();

        let _ = write!(
            sql,
            " ({}) VALUES ({})",
            cols.join(", "),
            placeholders.join(", ")
        );
        Rendered { sql, params }
    }
}

/// `UPDATE table SET ... WHERE ...` builder for entity `E`.
pub struct Update<E> {
    table: &'static str,
    assignments: Vec<Assignment>,
    filter: Option<Expr>,
    _marker: PhantomData<fn() -> E>,
}

impl<E> Update<E> {
    /// Start an UPDATE of `table`. Called by [`Entity::update`](crate::Entity).
    pub(crate) fn new(table: &'static str) -> Self {
        Update {
            table,
            assignments: Vec::new(),
            filter: None,
            _marker: PhantomData,
        }
    }

    /// Assign `column = value`. Repeat per column.
    pub fn set<T: ToSqlValue, V: Into<T>>(mut self, column: Column<E, T>, value: V) -> Self {
        self.assignments.push(Assignment {
            column: column.name,
            value: value.into().to_sql_value(),
        });
        self
    }

    /// Add a WHERE predicate. Multiple calls combine with `AND`.
    pub fn filter<S2, Idxs>(mut self, predicate: Predicate<S2>) -> Self
    where
        Scope<E>: ContainsAll<S2, Idxs>,
    {
        self.filter = Some(and_opt(self.filter.take(), predicate.into_expr()));
        self
    }

    /// Render for a specific dialect. SET values bind before WHERE parameters.
    pub fn render<D: Dialect>(&self, dialect: &D) -> Rendered {
        let mut params = Vec::new();
        let mut sql = String::from("UPDATE ");
        sql.push_str(&dialect.quote_ident(self.table));
        sql.push_str(" SET ");

        let sets: Vec<String> = self
            .assignments
            .iter()
            .map(|a| {
                params.push(a.value.clone());
                format!(
                    "{} = {}",
                    dialect.quote_ident(a.column),
                    dialect.placeholder(params.len())
                )
            })
            .collect();
        sql.push_str(&sets.join(", "));

        if let Some(filter) = &self.filter {
            sql.push_str(" WHERE ");
            sql.push_str(&render_expr(filter, dialect, &mut params));
        }
        Rendered { sql, params }
    }
}

/// `DELETE FROM table WHERE ...` builder for entity `E`.
pub struct Delete<E> {
    table: &'static str,
    filter: Option<Expr>,
    _marker: PhantomData<fn() -> E>,
}

impl<E> Delete<E> {
    /// Start a DELETE from `table`. Called by [`Entity::delete`](crate::Entity).
    pub(crate) fn new(table: &'static str) -> Self {
        Delete {
            table,
            filter: None,
            _marker: PhantomData,
        }
    }

    /// Add a WHERE predicate. Multiple calls combine with `AND`.
    ///
    /// A DELETE without a WHERE clause is allowed and truncates the table; be
    /// deliberate about it.
    pub fn filter<S2, Idxs>(mut self, predicate: Predicate<S2>) -> Self
    where
        Scope<E>: ContainsAll<S2, Idxs>,
    {
        self.filter = Some(and_opt(self.filter.take(), predicate.into_expr()));
        self
    }

    /// Render for a specific dialect.
    pub fn render<D: Dialect>(&self, dialect: &D) -> Rendered {
        let mut params = Vec::new();
        let mut sql = String::from("DELETE FROM ");
        sql.push_str(&dialect.quote_ident(self.table));

        if let Some(filter) = &self.filter {
            sql.push_str(" WHERE ");
            sql.push_str(&render_expr(filter, dialect, &mut params));
        }
        Rendered { sql, params }
    }
}

/// Combine an optional existing predicate with a new one using `AND`.
fn and_opt(existing: Option<Expr>, next: Expr) -> Expr {
    match existing {
        Some(prev) => Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(prev),
            rhs: Box::new(next),
        },
        None => next,
    }
}

//! Columns and tables: the query metamodel produced by `#[derive(Entity)]`.

use crate::expr::Expr;
use crate::expression::Expression;
use crate::sql_type::SqlType;
use core::marker::PhantomData;

/// A typed table column.
///
/// The SQL type is carried as a compile-time marker (`Sql`) so comparisons are
/// type-checked. `nullable` is tracked at runtime for the MVP; type-level
/// nullability tracking (so outer joins widen non-null columns) is planned for a
/// later phase.
pub struct Column<Sql> {
    /// Owning table name.
    pub table: &'static str,
    /// Column name.
    pub name: &'static str,
    /// Whether the column is nullable (derived from `Option<T>` fields).
    pub nullable: bool,
    _marker: PhantomData<Sql>,
}

impl<Sql> Column<Sql> {
    /// Construct a column. Called by generated metamodel code.
    pub const fn new(table: &'static str, name: &'static str, nullable: bool) -> Self {
        Column {
            table,
            name,
            nullable,
            _marker: PhantomData,
        }
    }
}

// Manual Clone/Copy so we do not require `Sql: Clone` (the marker is zero-sized
// and never instantiated).
impl<Sql> Clone for Column<Sql> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<Sql> Copy for Column<Sql> {}

impl<Sql: SqlType> Expression for Column<Sql> {
    type Sql = Sql;
    fn to_expr(&self) -> Expr {
        Expr::Column {
            table: self.table,
            name: self.name,
        }
    }
}

/// Metadata for a database table, implemented by the generated metamodel struct.
pub trait Table {
    /// The table's name as it appears in SQL.
    fn table_name(&self) -> &'static str;
}

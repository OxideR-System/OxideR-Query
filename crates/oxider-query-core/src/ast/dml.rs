//! The statement AST for INSERT, UPDATE, and DELETE.

use crate::ast::node::{Node, TableRef};
use crate::ast::query::{SelectAst, Source};

/// What to do when an INSERT hits a uniqueness conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictAction {
    /// Skip the row.
    DoNothing,
    /// Update the existing row with these assignments.
    DoUpdate(Vec<Assignment>),
}

/// An `ON CONFLICT` / `ON DUPLICATE KEY UPDATE` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct OnConflict {
    /// The conflicting columns. Empty means "any unique constraint", which is
    /// the only form MySQL supports.
    pub target: Vec<&'static str>,
    /// What to do about the conflict.
    pub action: ConflictAction,
}

/// A `column = expression` assignment in an INSERT's conflict action or an
/// UPDATE's SET clause.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// The assigned column.
    pub column: &'static str,
    /// The assigned value.
    pub value: Node,
}

/// Where the rows of an INSERT come from.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    /// Literal `VALUES` rows. Each row has one expression per named column.
    Values(Vec<Vec<Node>>),
    /// `INSERT ... SELECT`, taking rows from a query.
    Query(Box<SelectAst>),
}

/// A built INSERT statement.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertAst {
    /// The target table.
    pub table: TableRef,
    /// The inserted columns, in order.
    pub columns: Vec<&'static str>,
    /// Where the rows come from.
    pub source: InsertSource,
    /// Conflict handling, if any.
    pub on_conflict: Option<OnConflict>,
    /// `RETURNING` expressions; empty means no RETURNING clause.
    pub returning: Vec<Node>,
}

/// A built UPDATE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateAst {
    /// The target table.
    pub table: TableRef,
    /// The SET assignments, in order.
    pub assignments: Vec<Assignment>,
    /// Extra sources joined into the update (`UPDATE ... FROM`).
    pub from: Vec<Source>,
    /// The WHERE predicate.
    pub filter: Option<Node>,
    /// `RETURNING` expressions.
    pub returning: Vec<Node>,
}

/// A built DELETE statement.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteAst {
    /// The target table.
    pub table: TableRef,
    /// Extra sources referenced by the predicate (`DELETE ... USING`).
    pub using: Vec<Source>,
    /// The WHERE predicate.
    pub filter: Option<Node>,
    /// `RETURNING` expressions.
    pub returning: Vec<Node>,
}

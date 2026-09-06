//! The statement AST: SELECT and its clauses, plus the shared clause types the
//! DML statements reuse.
//!
//! Everything here is plain data with no type parameters. The typed builders in
//! [`crate::builder`] enforce the compile-time rules and then lower into these
//! structs, which the renderer turns into `(sql, params)`.

use crate::ast::node::{Node, TableRef};
use crate::ast::window::{InlineWindow, WindowSpec};

/// Sort direction for an ORDER BY term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    /// Ascending (`ASC`).
    Asc,
    /// Descending (`DESC`).
    Desc,
}

impl OrderDir {
    /// The SQL keyword for this direction.
    pub fn as_sql(self) -> &'static str {
        match self {
            OrderDir::Asc => "ASC",
            OrderDir::Desc => "DESC",
        }
    }
}

/// Where NULLs sort relative to non-NULL values.
///
/// Dialects without native `NULLS FIRST`/`NULLS LAST` (MySQL, SQLite, SQL
/// Server) get a `CASE WHEN <expr> IS NULL THEN 0 ELSE 1 END` sort key emitted
/// ahead of the term, which is how QueryDSL emulates it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullsOrder {
    /// Leave it to the engine's default.
    Default,
    /// `NULLS FIRST`.
    First,
    /// `NULLS LAST`.
    Last,
}

/// One ORDER BY term.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderAst {
    /// The sorted expression.
    pub expr: Node,
    /// Sort direction.
    pub dir: OrderDir,
    /// Null placement.
    pub nulls: NullsOrder,
}

impl OrderAst {
    /// An order term with engine-default null placement.
    pub fn new(expr: Node, dir: OrderDir) -> Self {
        OrderAst {
            expr,
            dir,
            nulls: NullsOrder::Default,
        }
    }
}

/// The kind of join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// `INNER JOIN`
    Inner,
    /// `LEFT JOIN`
    Left,
    /// `RIGHT JOIN`
    Right,
    /// `FULL JOIN`
    Full,
    /// `CROSS JOIN` - no ON clause.
    Cross,
}

impl JoinKind {
    /// The SQL keyword sequence for this join kind.
    pub fn as_sql(self) -> &'static str {
        match self {
            JoinKind::Inner => "INNER JOIN",
            JoinKind::Left => "LEFT JOIN",
            JoinKind::Right => "RIGHT JOIN",
            JoinKind::Full => "FULL JOIN",
            JoinKind::Cross => "CROSS JOIN",
        }
    }

    /// Whether an outer join, so columns of the joined table can be NULL.
    pub fn is_outer(self) -> bool {
        matches!(self, JoinKind::Left | JoinKind::Right | JoinKind::Full)
    }
}

/// Something that can appear in FROM or as a join target.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A table, optionally aliased.
    Table(TableRef),
    /// A subquery in FROM, which must be aliased (`derived table`).
    Derived {
        /// The subquery.
        query: Box<SelectAst>,
        /// The mandatory alias.
        alias: &'static str,
    },
}

/// A single JOIN clause.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinAst {
    /// The join kind.
    pub kind: JoinKind,
    /// The joined source.
    pub source: Source,
    /// The ON condition; always `None` for `CROSS JOIN`.
    pub on: Option<Node>,
}

/// The DISTINCT modifier on a SELECT.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Distinct {
    /// No DISTINCT.
    #[default]
    No,
    /// `SELECT DISTINCT`.
    All,
    /// `SELECT DISTINCT ON (...)`, a PostgreSQL extension.
    On(Vec<Node>),
}

/// A set operation combining two queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    /// `UNION` - duplicates removed.
    Union,
    /// `UNION ALL` - duplicates kept.
    UnionAll,
    /// `INTERSECT`
    Intersect,
    /// `INTERSECT ALL`
    IntersectAll,
    /// `EXCEPT`
    Except,
    /// `EXCEPT ALL`
    ExceptAll,
}

impl SetOp {
    /// The SQL keyword sequence for this set operation.
    pub fn as_sql(self) -> &'static str {
        match self {
            SetOp::Union => "UNION",
            SetOp::UnionAll => "UNION ALL",
            SetOp::Intersect => "INTERSECT",
            SetOp::IntersectAll => "INTERSECT ALL",
            SetOp::Except => "EXCEPT",
            SetOp::ExceptAll => "EXCEPT ALL",
        }
    }
}

/// A row-locking clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockMode {
    /// `FOR UPDATE`
    Update,
    /// `FOR SHARE`
    Share,
    /// `FOR NO KEY UPDATE` (PostgreSQL).
    NoKeyUpdate,
    /// `FOR KEY SHARE` (PostgreSQL).
    KeyShare,
}

/// How a locking clause behaves when a row is already locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockWait {
    /// Block until the lock is available.
    Wait,
    /// `NOWAIT` - fail immediately.
    NoWait,
    /// `SKIP LOCKED` - omit locked rows.
    SkipLocked,
}

/// The row-locking clause of a SELECT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lock {
    /// The lock strength.
    pub mode: LockMode,
    /// The contention behaviour.
    pub wait: LockWait,
}

/// One common table expression in a `WITH` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    /// The CTE name, used as a table name in the main query.
    pub name: &'static str,
    /// Optional explicit column names.
    pub columns: Vec<&'static str>,
    /// The CTE body.
    pub query: Box<SelectAst>,
}

/// A built SELECT statement in dialect-agnostic form.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectAst {
    /// `WITH` common table expressions.
    pub with: Vec<Cte>,
    /// Whether the `WITH` clause is `RECURSIVE`.
    pub recursive: bool,
    /// The DISTINCT modifier.
    pub distinct: Distinct,
    /// Selected expressions. Empty means `SELECT *`.
    pub columns: Vec<Node>,
    /// FROM sources, in order. Empty means a FROM-less SELECT.
    pub from: Vec<Source>,
    /// JOIN clauses, in order.
    pub joins: Vec<JoinAst>,
    /// WHERE predicate.
    pub filter: Option<Node>,
    /// GROUP BY expressions.
    pub group: Vec<Node>,
    /// HAVING predicate.
    pub having: Option<Node>,
    /// Named windows declared in a `WINDOW` clause.
    pub windows: Vec<(&'static str, InlineWindow)>,
    /// Set operations applied to this query, in order.
    pub set_ops: Vec<(SetOp, Box<SelectAst>)>,
    /// ORDER BY terms.
    pub order: Vec<OrderAst>,
    /// LIMIT.
    pub limit: Option<u64>,
    /// OFFSET.
    pub offset: Option<u64>,
    /// Row-locking clause.
    pub lock: Option<Lock>,
}

impl SelectAst {
    /// A SELECT over a single table.
    pub fn from_table(table: TableRef) -> Self {
        SelectAst {
            from: vec![Source::Table(table)],
            ..Default::default()
        }
    }

    /// Whether this query, or any set-operation branch of it, uses a window
    /// specification. Lets the renderer decide whether a dialect capability
    /// check is needed.
    pub fn references_window(&self) -> bool {
        !self.windows.is_empty() || self.columns.iter().any(node_has_window)
    }
}

fn node_has_window(node: &Node) -> bool {
    match node {
        Node::Window { .. } => true,
        Node::Op(_, args) | Node::Row(args) => args.iter().any(node_has_window),
        Node::Alias(inner, _) => node_has_window(inner),
        _ => false,
    }
}

/// A window reference resolved for rendering: either the inline definition or a
/// name declared in the query's `WINDOW` clause.
pub type ResolvedWindow<'a> = &'a WindowSpec;

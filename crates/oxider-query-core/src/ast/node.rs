//! The dialect-agnostic expression AST.
//!
//! This is the untyped intermediate representation every typed builder lowers
//! into. Type-safety lives one layer up (see [`crate::typed`]); keeping the AST
//! dynamic is what lets one query render to several dialects, and it is the
//! same seam QueryDSL uses - there, an `Expression` tree is walked by a
//! `Visitor`, and swapping the visitor is what makes the same tree serialize to
//! SQL, to JPQL, or to a MongoDB query document.
//!
//! Identifiers are `&'static str` so the metamodel can build columns in `const`
//! context and stay `Copy`.

use crate::ast::operator::Operator;
use crate::ast::query::SelectAst;
use crate::ast::window::WindowSpec;
use crate::value::Value;

/// A table reference: optional schema, name, and optional alias.
///
/// The alias is what makes self-joins and correlated subqueries expressible;
/// `users AS u` and `users AS m` are two distinct references to one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableRef {
    /// Schema qualifier, when the table is not in the default schema.
    pub schema: Option<&'static str>,
    /// The table name as it appears in SQL.
    pub name: &'static str,
    /// Alias introduced in FROM/JOIN, used to qualify column references.
    pub alias: Option<&'static str>,
}

impl TableRef {
    /// A reference to `name` in the default schema, unaliased.
    pub const fn new(name: &'static str) -> Self {
        TableRef {
            schema: None,
            name,
            alias: None,
        }
    }

    /// The same table under an alias.
    pub const fn aliased(name: &'static str, alias: &'static str) -> Self {
        TableRef {
            schema: None,
            name,
            alias: Some(alias),
        }
    }

    /// The identifier columns of this table are qualified with: the alias when
    /// there is one, else the table name.
    pub const fn qualifier(&self) -> &'static str {
        match self.alias {
            Some(alias) => alias,
            None => self.name,
        }
    }
}

/// A qualified column reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnRef {
    /// The table the column belongs to.
    pub table: TableRef,
    /// The column name.
    pub name: &'static str,
}

/// One `WHEN cond THEN result` arm of a CASE expression.
#[derive(Debug, Clone, PartialEq)]
pub struct WhenArm {
    /// The condition (or, for a simple CASE, the value compared against the
    /// operand).
    pub when: Node,
    /// The result when this arm matches.
    pub then: Node,
}

/// A piece of a raw SQL fragment: either verbatim SQL text or an embedded
/// expression, which is rendered (and parameterized) normally.
///
/// Splitting the fragment this way is what keeps the escape hatch safe: user
/// values travel through [`Node`] and become bind parameters, never string
/// concatenation into the SQL text.
#[derive(Debug, Clone, PartialEq)]
pub enum RawPart {
    /// Verbatim SQL, written by the developer.
    Sql(&'static str),
    /// An embedded expression, rendered in place.
    Expr(Node),
}

/// An untyped expression node.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A qualified column reference.
    Column(ColumnRef),
    /// A bound parameter carrying its value.
    Param(Value),
    /// A named bind parameter, resolved before execution.
    NamedParam(&'static str),
    /// A verbatim SQL keyword or literal that is never user data, such as
    /// `CURRENT_TIMESTAMP` or `NULL`.
    Keyword(&'static str),
    /// `*`, optionally qualified by a table (`t.*`).
    Star(Option<TableRef>),
    /// An operator applied to its arguments. The single node that carries the
    /// whole operator catalog; rendering looks the operator up in the dialect's
    /// template table.
    Op(Operator, Vec<Node>),
    /// An aggregate function call, which unlike a plain operator can carry
    /// `DISTINCT` and a `FILTER (WHERE ...)` clause.
    Aggregate {
        /// The aggregate operator (`COUNT`, `SUM`, ...).
        func: Operator,
        /// Whether the argument list is `DISTINCT`.
        distinct: bool,
        /// The aggregated expressions; empty for `COUNT(*)`.
        args: Vec<Node>,
        /// `ORDER BY` inside the call, for ordered aggregates such as
        /// string aggregation.
        order_by: Vec<crate::ast::query::OrderAst>,
        /// Optional `FILTER (WHERE ...)` restricting the aggregated rows.
        filter: Option<Box<Node>>,
    },
    /// `CAST(expr AS <type>)`. The target type is named by the dialect at
    /// render time, so one query casts correctly on every engine.
    Cast {
        /// The expression being cast.
        expr: Box<Node>,
        /// The target type, resolved per dialect.
        kind: crate::dialect::CastKind,
    },
    /// A `CASE` expression. `operand` is `Some` for the simple form
    /// (`CASE x WHEN 1 THEN ...`) and `None` for the searched form.
    Case {
        /// The compared operand for a simple CASE.
        operand: Option<Box<Node>>,
        /// The `WHEN ... THEN ...` arms, in order.
        arms: Vec<WhenArm>,
        /// The `ELSE` result, if any.
        otherwise: Option<Box<Node>>,
    },
    /// A window function call: an aggregate or ranking function plus its
    /// `OVER (...)` specification.
    Window {
        /// The function being windowed.
        func: Box<Node>,
        /// The window specification, or a reference to a named window.
        spec: Box<WindowSpec>,
    },
    /// A parenthesized subquery, used as a scalar, as the right side of `IN`,
    /// or inside `EXISTS`.
    Subquery(Box<SelectAst>),
    /// A row constructor, `(a, b, c)`. Used for row comparisons and for the
    /// value list of `IN`.
    Row(Vec<Node>),
    /// The value a conflicting INSERT tried to write, inside an upsert's update
    /// clause. Spelled `excluded.<column>` by PostgreSQL and SQLite and
    /// `VALUES(<column>)` by MySQL, so the dialect resolves it at render time.
    Excluded(&'static str),
    /// `expr AS alias` in a SELECT list.
    Alias(Box<Node>, &'static str),
    /// A raw SQL fragment with embedded, parameterized expressions.
    Raw(Vec<RawPart>),
}

impl Node {
    /// Build an operator node from an iterator of arguments.
    pub fn op(operator: Operator, args: impl IntoIterator<Item = Node>) -> Node {
        Node::Op(operator, args.into_iter().collect())
    }

    /// Build a binary operator node.
    pub fn binary(operator: Operator, lhs: Node, rhs: Node) -> Node {
        Node::Op(operator, vec![lhs, rhs])
    }

    /// Build a unary operator node.
    pub fn unary(operator: Operator, arg: Node) -> Node {
        Node::Op(operator, vec![arg])
    }

    /// Combine two operands with an associative connective, flattening rather
    /// than nesting.
    ///
    /// `a AND b AND c` is one node with three arguments, not two nodes. The
    /// rendered SQL is identical either way - the connectives are associative
    /// and share a precedence level, so no parentheses appear in either shape -
    /// but the depth is what matters: a filter accumulated in a loop stays two
    /// levels deep instead of growing one level per condition, which is the
    /// difference between a query that renders and one that used to walk
    /// thousands of stack frames.
    pub fn connect(operator: Operator, mut lhs: Node, rhs: Node) -> Node {
        match &mut lhs {
            Node::Op(op, args) if *op == operator => {
                args.push(rhs);
                lhs
            }
            _ => Node::Op(operator, vec![lhs, rhs]),
        }
    }

    /// Combine two optional predicates with `AND`, keeping `None` absorbing.
    ///
    /// Used by every clause that accumulates predicates across repeated calls
    /// (`where`, `having`, `on`).
    pub fn and_opt(existing: Option<Node>, next: Node) -> Node {
        match existing {
            Some(prev) => Node::connect(Operator::And, prev, next),
            None => next,
        }
    }
}

/// Tear a node down iteratively, so a deep tree cannot overflow the stack.
///
/// The derived drop glue is recursive: freeing an expression a few thousand
/// levels deep walks a stack frame per level and aborts the process, which no
/// caller can catch or recover from. Detaching children into a worklist first
/// means every node this runs on is already childless by the time its own drop
/// glue runs, so the recursion is one level deep whatever the tree looks like.
impl Drop for Node {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        detach_children(self, &mut pending);
        while let Some(mut node) = pending.pop() {
            detach_children(&mut node, &mut pending);
        }
    }
}

/// Move every child node of `node` into `out`, leaving `node` childless.
fn detach_children(node: &mut Node, out: &mut Vec<Node>) {
    /// A leaf to swap in for a child being taken away. Never rendered: the
    /// node holding it is on its way out.
    fn stub() -> Node {
        Node::Keyword("")
    }

    match node {
        Node::Column(_)
        | Node::Param(_)
        | Node::NamedParam(_)
        | Node::Keyword(_)
        | Node::Star(_)
        | Node::Excluded(_) => {}
        Node::Op(_, args) | Node::Row(args) => out.append(args),
        Node::Aggregate {
            args,
            order_by,
            filter,
            ..
        } => {
            out.append(args);
            out.extend(order_by.drain(..).map(|term| term.expr));
            out.extend(filter.take().map(|boxed| *boxed));
        }
        Node::Cast { expr, .. } => out.push(core::mem::replace(&mut **expr, stub())),
        Node::Case {
            operand,
            arms,
            otherwise,
        } => {
            out.extend(operand.take().map(|boxed| *boxed));
            out.extend(arms.drain(..).flat_map(|arm| [arm.when, arm.then]));
            out.extend(otherwise.take().map(|boxed| *boxed));
        }
        Node::Window { func, .. } => out.push(core::mem::replace(&mut **func, stub())),
        // A subquery owns a whole statement, not a node. Its own expressions
        // each drop through here, so an expression of any depth inside a
        // subquery is safe; what stays recursive is statements nested inside
        // statements, which no accumulation loop produces - every level needs a
        // separately built query - and which the renderer's depth guard refuses
        // long before it matters.
        Node::Subquery(_) => {}
        Node::Alias(inner, _) => out.push(core::mem::replace(&mut **inner, stub())),
        Node::Raw(parts) => out.extend(parts.drain(..).filter_map(|part| match part {
            RawPart::Sql(_) => None,
            RawPart::Expr(node) => Some(node),
        })),
    }
}

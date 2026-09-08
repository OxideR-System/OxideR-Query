//! The SELECT builder.
//!
//! [`Select`] carries two type-level sets and no runtime overhead beyond the
//! AST it accumulates:
//!
//! - `S`, everything in scope: the FROM entity, every joined entity, and any
//!   outer entity brought in by [`correlate`](Select::correlate). Clauses check
//!   their expressions against it.
//! - `F`, the outer entities this query is free in. Empty for a stand-alone
//!   query; non-empty for a correlated subquery, where it becomes the source
//!   set of the resulting expression so the *outer* query checks those entities
//!   are in ITS scope.
//!
//! Splitting the two is what makes correlated subqueries type-check. Merging
//! them would make the outer query demand the subquery's own tables, and
//! keeping only `S` would let a correlated reference escape unchecked.

use crate::ast::node::{Node, TableRef};
use crate::ast::query::{
    Cte, Distinct, JoinAst, JoinKind, Lock, LockMode, LockWait, SelectAst, SetOp, Source,
};
use crate::builder::subquery::Subquery;
use crate::dialect::Dialect;
use crate::render::{render_select_into, Bindings, RenderResult, Renderable, Rendered};
use crate::source::{Cons, ContainsAll, Nil};
use crate::typed::expr::{Order, Predicate};
use crate::typed::selection::{AnyExpr, SelectionIn};
use crate::typed::window::Window;
use crate::typed::{Column, Entity, Table};
use core::marker::PhantomData;

/// A query with no row-locking clause. The state every SELECT starts in.
pub struct Unlocked;

/// A query carrying a row-locking clause.
///
/// The wait policy - `NOWAIT`, `SKIP LOCKED` - is only meaningful as a modifier
/// of a lock, so [`no_wait`](Select::no_wait) and
/// [`skip_locked`](Select::skip_locked) exist only in this state. Asking for
/// `SKIP LOCKED` without a lock used to render a query with no locking at all,
/// which in the queue-worker pattern those methods exist for means several
/// workers quietly processing the same rows.
pub struct Locked;

/// The zero-sized marker carrying a query's three type parameters.
///
/// Named rather than written inline so the struct definition stays readable
/// with three of them; `fn() -> _` keeps `Select` covariant and imposes no
/// auto-trait bounds on the parameters.
type Markers<S, F, L> = PhantomData<fn() -> (S, F, L)>;

/// A SELECT statement under construction.
pub struct Select<S, F = Nil, L = Unlocked> {
    ast: SelectAst,
    bindings: Bindings,
    _marker: Markers<S, F, L>,
}

impl<S, F, L> Select<S, F, L> {
    /// Start a SELECT over one table. The caller states the scope.
    pub(crate) fn new(table: TableRef) -> Self {
        Select {
            ast: SelectAst::from_table(table),
            bindings: Bindings::new(),
            _marker: PhantomData,
        }
    }

    /// Rebuild with a different type-level scope, keeping the AST.
    fn retype<S2, F2, L2>(self) -> Select<S2, F2, L2> {
        Select {
            ast: self.ast,
            bindings: self.bindings,
            _marker: PhantomData,
        }
    }

    /// The accumulated statement.
    pub fn into_ast(self) -> SelectAst {
        self.ast
    }

    /// Borrow the accumulated statement.
    pub fn as_ast(&self) -> &SelectAst {
        &self.ast
    }

    /// Render for a dialect.
    pub fn to_sql(&self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_select_into(&self.ast, dialect, &self.bindings)
    }

    /// Give a named parameter its value.
    ///
    /// The counterpart to [`param`](crate::typed::param): a statement is built
    /// once with placeholders and rendered as often as needed, one value set at
    /// a time. Binding the same name twice keeps the last value, and a name
    /// left unbound is a render error rather than a silently missing value.
    /// Bindings resolve for the whole statement, so a parameter inside a
    /// subquery is bound here too.
    pub fn bind(mut self, name: &'static str, value: impl Into<crate::value::Value>) -> Self {
        self.bindings = core::mem::take(&mut self.bindings).set(name, value);
        self
    }
}

// --- Sources -------------------------------------------------------------

impl<S, F, L> Select<S, F, L> {
    /// Add another table to FROM, joined by the WHERE clause rather than by an
    /// explicit JOIN. The older spelling of a cross join.
    pub fn and_from<E2>(mut self, table: Table<E2>) -> Select<Cons<E2, S>, F, L> {
        self.ast.from.push(Source::Table(table.reference()));
        self.retype()
    }

    /// Add a subquery to FROM as a derived table.
    ///
    /// Its columns have no metamodel behind them, so they are reached with
    /// [`col`](crate::typed::col) under the alias given here, and the compiler
    /// cannot check them.
    pub fn and_from_query<S2, F2, L2>(
        mut self,
        query: Select<S2, F2, L2>,
        alias: &'static str,
    ) -> Select<S, F, L> {
        self.ast.from.push(Source::Derived {
            query: Box::new(query.into_ast()),
            alias,
        });
        self
    }

    /// Bring an outer query's entity into scope, making this a correlated
    /// subquery.
    ///
    /// The entity joins the local scope, so the subquery may reference its
    /// columns, and joins the free set, so whoever embeds this subquery has to
    /// have that entity in scope. Call it before the clause that references the
    /// outer table.
    pub fn correlate<E2>(self) -> Select<Cons<E2, S>, Cons<E2, F>, L> {
        self.retype()
    }
}

/// Generate a join method for one join kind.
macro_rules! join_method {
    ($name:ident, $kind:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<E2, S2, I>(
            mut self,
            table: Table<E2>,
            on: Predicate<S2>,
        ) -> Select<Cons<E2, S>, F, L>
        where
            Cons<E2, S>: ContainsAll<S2, I>,
        {
            self.ast.joins.push(JoinAst {
                kind: $kind,
                source: Source::Table(table.reference()),
                on: Some(on.into_node()),
            });
            self.retype()
        }
    };
}

impl<S, F, L> Select<S, F, L> {
    join_method!(
        inner_join,
        JoinKind::Inner,
        "`INNER JOIN table ON condition`. The condition may reference the joined \
         entity, which is in scope for it."
    );
    join_method!(left_join, JoinKind::Left, "`LEFT JOIN table ON condition`.");
    join_method!(
        right_join,
        JoinKind::Right,
        "`RIGHT JOIN table ON condition`. Rejected at render time on SQLite."
    );
    join_method!(
        full_join,
        JoinKind::Full,
        "`FULL JOIN table ON condition`. Rejected at render time on MySQL and \
         SQLite."
    );

    /// `CROSS JOIN table` - every combination, with no condition.
    pub fn cross_join<E2>(mut self, table: Table<E2>) -> Select<Cons<E2, S>, F, L> {
        self.ast.joins.push(JoinAst {
            kind: JoinKind::Cross,
            source: Source::Table(table.reference()),
            on: None,
        });
        self.retype()
    }

    /// Add a named source to FROM, with no entity behind it.
    ///
    /// The counterpart to [`select_from_name`]: what a CTE or a view needs.
    pub fn and_from_name(mut self, name: &'static str) -> Self {
        self.ast.from.push(Source::Table(TableRef::new(name)));
        self
    }

    /// Join a named source, with no entity behind it.
    ///
    /// A CTE has to be joined this way rather than wrapped in a subquery: a
    /// recursive CTE may reference itself only directly in `FROM`, so
    /// [`join_query`](Select::join_query) would make it a circular reference.
    pub fn join_name<S2, I>(self, name: &'static str, on: Predicate<S2>) -> Select<S, F, L>
    where
        S: ContainsAll<S2, I>,
    {
        self.join_named(TableRef::new(name), on)
    }

    /// Join a named source under an alias.
    pub fn join_name_as<S2, I>(
        self,
        name: &'static str,
        alias: &'static str,
        on: Predicate<S2>,
    ) -> Select<S, F, L>
    where
        S: ContainsAll<S2, I>,
    {
        self.join_named(TableRef::aliased(name, alias), on)
    }

    fn join_named<S2>(mut self, table: TableRef, on: Predicate<S2>) -> Select<S, F, L> {
        self.ast.joins.push(JoinAst {
            kind: JoinKind::Inner,
            source: Source::Table(table),
            on: Some(on.into_node()),
        });
        self
    }

    /// Join a subquery as a derived table. Its columns are reached with
    /// [`col`](crate::typed::col) under `alias`.
    pub fn join_query<S2, F2, L2, S3, I>(
        mut self,
        query: Select<S2, F2, L2>,
        alias: &'static str,
        on: Predicate<S3>,
    ) -> Select<S, F, L>
    where
        S: ContainsAll<S3, I>,
    {
        self.ast.joins.push(JoinAst {
            kind: JoinKind::Inner,
            source: Source::Derived {
                query: Box::new(query.into_ast()),
                alias,
            },
            on: Some(on.into_node()),
        });
        self
    }
}

// --- Filtering and projection --------------------------------------------

impl<S, F, L> Select<S, F, L> {
    /// Add a WHERE condition. Repeated calls are combined with `AND`.
    pub fn filter<S2, I>(mut self, predicate: Predicate<S2>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        self.ast.filter = Some(Node::and_opt(self.ast.filter.take(), predicate.into_node()));
        self
    }

    /// Add a WHERE condition only when there is one.
    ///
    /// The building block for search forms, where a filter applies only if the
    /// user filled the field in. QueryDSL reaches the same place by having
    /// `where(null)` be a no-op; making the option explicit keeps a genuine
    /// mistake from silently widening the result set.
    pub fn filter_opt<S2, I>(self, predicate: Option<Predicate<S2>>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        match predicate {
            Some(predicate) => self.filter(predicate),
            None => self,
        }
    }

    /// Set the projection. Replaces any previous one.
    ///
    /// Takes a single expression or a tuple of up to twelve.
    pub fn select<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<S, Idxs>,
    {
        self.ast.columns.clear();
        selection.append_nodes(&mut self.ast.columns);
        self
    }

    /// Add expressions to the projection, keeping what is already there.
    pub fn add_select<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<S, Idxs>,
    {
        selection.append_nodes(&mut self.ast.columns);
        self
    }

    /// `SELECT DISTINCT`.
    pub fn distinct(mut self) -> Self {
        self.ast.distinct = Distinct::All;
        self
    }

    /// `SELECT DISTINCT ON (...)` - one row per distinct key, the first in the
    /// query's own ordering. A PostgreSQL extension; rejected elsewhere.
    pub fn distinct_on<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<S, Idxs>,
    {
        let mut keys = Vec::new();
        selection.append_nodes(&mut keys);
        self.ast.distinct = Distinct::On(keys);
        self
    }

    /// Add GROUP BY expressions.
    pub fn group_by<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<S, Idxs>,
    {
        selection.append_nodes(&mut self.ast.group);
        self
    }

    /// Add a HAVING condition. Repeated calls are combined with `AND`.
    pub fn having<S2, I>(mut self, predicate: Predicate<S2>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        self.ast.having = Some(Node::and_opt(self.ast.having.take(), predicate.into_node()));
        self
    }

    /// Declare a named window, referenced by
    /// [`over_named`](crate::typed::Aggregate::over_named).
    pub fn window<S2, Fr>(mut self, name: &'static str, window: Window<S2, Fr>) -> Self {
        self.ast.windows.push((name, window.into_definition()));
        self
    }
}

// --- Ordering, paging, locking -------------------------------------------

impl<S, F, L> Select<S, F, L> {
    /// Add an ORDER BY term. Repeated calls append, so the first call is the
    /// primary sort.
    pub fn order_by<S2, I>(mut self, term: Order<S2>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        self.ast.order.push(term.into_term());
        self
    }

    /// Add several ORDER BY terms sharing one scope.
    pub fn order_by_all<S2, I>(mut self, terms: impl IntoIterator<Item = Order<S2>>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        self.ast
            .order
            .extend(terms.into_iter().map(Order::into_term));
        self
    }

    /// Add an ORDER BY term only when there is one.
    pub fn order_by_opt<S2, I>(self, term: Option<Order<S2>>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        match term {
            Some(term) => self.order_by(term),
            None => self,
        }
    }

    /// Cap the number of rows.
    pub fn limit(mut self, rows: u64) -> Self {
        self.ast.limit = Some(rows);
        self
    }

    /// Skip rows before returning any.
    pub fn offset(mut self, rows: u64) -> Self {
        self.ast.offset = Some(rows);
        self
    }

    /// Take one page of `size` rows, counting pages from zero.
    pub fn page(self, page: u64, size: u64) -> Self {
        self.limit(size).offset(page.saturating_mul(size))
    }
}

// --- Locking -------------------------------------------------------------
//
// Taking a lock moves the query into the `Locked` state, and the wait policy
// exists only there. The alternative - accepting `skip_locked()` on any query
// and ignoring it when there is no lock - renders a query with no locking at
// all, which is exactly wrong for the pattern those methods serve.

impl<S, F> Select<S, F, Unlocked> {
    /// `FOR UPDATE` - lock the selected rows for writing.
    pub fn for_update(self) -> Select<S, F, Locked> {
        self.lock(LockMode::Update)
    }

    /// `FOR SHARE` - lock the selected rows against writers.
    pub fn for_share(self) -> Select<S, F, Locked> {
        self.lock(LockMode::Share)
    }

    /// `FOR NO KEY UPDATE` - a weaker `FOR UPDATE` that still allows foreign
    /// keys to reference the row. PostgreSQL only.
    pub fn for_no_key_update(self) -> Select<S, F, Locked> {
        self.lock(LockMode::NoKeyUpdate)
    }

    /// `FOR KEY SHARE` - the weakest lock. PostgreSQL only.
    pub fn for_key_share(self) -> Select<S, F, Locked> {
        self.lock(LockMode::KeyShare)
    }

    fn lock(mut self, mode: LockMode) -> Select<S, F, Locked> {
        self.ast.lock = Some(Lock {
            mode,
            wait: LockWait::Wait,
        });
        self.retype()
    }
}

impl<S, F> Select<S, F, Locked> {
    /// Fail rather than wait when a selected row is already locked.
    pub fn no_wait(self) -> Self {
        self.wait(LockWait::NoWait)
    }

    /// Skip rows that are already locked - the queue-worker pattern.
    pub fn skip_locked(self) -> Self {
        self.wait(LockWait::SkipLocked)
    }

    fn wait(mut self, policy: LockWait) -> Self {
        let lock = self
            .ast
            .lock
            .as_mut()
            .expect("a Locked query always carries a lock");
        lock.wait = policy;
        self
    }
}

// --- CTEs and set operations ---------------------------------------------

impl<S, F, L> Select<S, F, L> {
    /// Prepend a common table expression.
    ///
    /// Its columns are reached with [`col`](crate::typed::col) under `name`.
    pub fn with<S2, F2, L2>(mut self, name: &'static str, query: Select<S2, F2, L2>) -> Self {
        self.ast.with.push(Cte {
            name,
            columns: Vec::new(),
            query: Box::new(query.into_ast()),
        });
        self
    }

    /// Prepend a common table expression with explicit column names.
    pub fn with_columns<S2, F2, L2>(
        mut self,
        name: &'static str,
        columns: impl IntoIterator<Item = &'static str>,
        query: Select<S2, F2, L2>,
    ) -> Self {
        self.ast.with.push(Cte {
            name,
            columns: columns.into_iter().collect(),
            query: Box::new(query.into_ast()),
        });
        self
    }

    /// Mark the `WITH` clause `RECURSIVE`, so a CTE may refer to itself.
    pub fn recursive(mut self) -> Self {
        self.ast.recursive = true;
        self
    }
}

/// Generate a set-operation method.
macro_rules! set_op_method {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        pub fn $name<S2, F2, L2>(mut self, other: Select<S2, F2, L2>) -> Self {
            self.ast.set_ops.push(($op, Box::new(other.into_ast())));
            self
        }
    };
}

impl<S, F, L> Select<S, F, L> {
    set_op_method!(
        union,
        SetOp::Union,
        "`UNION` - rows of both queries, duplicates removed."
    );
    set_op_method!(
        union_all,
        SetOp::UnionAll,
        "`UNION ALL` - rows of both queries, duplicates kept."
    );
    set_op_method!(
        intersect,
        SetOp::Intersect,
        "`INTERSECT` - rows in both queries."
    );
    set_op_method!(
        intersect_all,
        SetOp::IntersectAll,
        "`INTERSECT ALL`, keeping duplicate multiplicity."
    );
    set_op_method!(
        except,
        SetOp::Except,
        "`EXCEPT` - rows of this query not in the other."
    );
    set_op_method!(
        except_all,
        SetOp::ExceptAll,
        "`EXCEPT ALL`, keeping duplicate multiplicity."
    );
}

// --- Becoming an expression ----------------------------------------------

impl<S, F, L> Select<S, F, L> {
    /// Turn this query into a scalar subquery selecting one expression.
    ///
    /// The result's source set is `F`, this query's free entities, so an
    /// uncorrelated subquery is usable anywhere and a correlated one only where
    /// the outer entity is in scope.
    pub fn scalar<X, I>(mut self, expr: X) -> Subquery<F, X::Output>
    where
        X: AnyExpr,
        S: ContainsAll<X::Sources, I>,
    {
        self.ast.columns.clear();
        self.ast.columns.push(expr.into_any_node());
        Subquery::new(self.ast)
    }

    /// Turn this query into a subquery over its existing projection, for use on
    /// the right of `IN`.
    pub fn as_subquery<T>(self) -> Subquery<F, T> {
        Subquery::new(self.ast)
    }
}

impl<S, F, L> Renderable for Select<S, F, L> {
    fn render_with(self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_select_into(&self.ast, dialect, &self.bindings)
    }
}

impl<S, F, L> Clone for Select<S, F, L> {
    fn clone(&self) -> Self {
        Select {
            ast: self.ast.clone(),
            bindings: self.bindings.clone(),
            _marker: PhantomData,
        }
    }
}

impl<S, F, L> core::fmt::Debug for Select<S, F, L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Select").field(&self.ast).finish()
    }
}

/// Start a SELECT with no FROM clause, for `SELECT 1` and similar probes.
pub fn select_only<Sel, Idxs>(selection: Sel) -> Select<Nil>
where
    Sel: SelectionIn<Nil, Idxs>,
{
    let mut ast = SelectAst::default();
    selection.append_nodes(&mut ast.columns);
    Select {
        ast,
        bindings: Bindings::new(),
        _marker: PhantomData,
    }
}

/// Start a SELECT over a table named directly, with no entity behind it.
///
/// The counterpart to [`col`](crate::typed::col): what a query over a CTE, a
/// view, or a table added after codegen ran needs. The scope is empty, so every
/// column reference goes through `col` and the compiler checks nothing about
/// them.
pub fn select_from_name(name: &'static str) -> Select<Nil> {
    Select::new(TableRef::new(name))
}

/// Start a SELECT over a table reference, when the entity is known but the
/// alias is chosen at the call site.
pub fn select_from<E: Entity>(table: Table<E>) -> Select<Cons<E, Nil>> {
    Select::new(table.reference())
}

/// The columns of `E`, for `SELECT t.*`.
///
/// A convenience over [`star_of`](crate::typed::star_of) that takes the
/// qualifier from the column's own table rather than a written string.
pub fn all_of<E, T>(column: Column<E, T>) -> crate::typed::Expr<Cons<E, Nil>, ()> {
    crate::typed::Expr::new(Node::Star(Some(column.table)))
}

//! The SELECT query builder and its dialect-agnostic output AST.
//!
//! [`Select<S>`] tracks, at the type level, the set of entities in scope: the
//! FROM entity plus every joined entity (see [`crate::source`]). Clause methods
//! that reference columns - `select`, `filter`, `order_by` - require the
//! referenced entities to be contained in `S`, so referencing a column of a
//! table you forgot to join is a compile error. `join` extends `S` with the
//! newly joined entity.

use crate::column::{Column, Entity};
use crate::dialect::Dialect;
use crate::expr::{BinOp, Expr};
use crate::join::{Join, JoinKind};
use crate::predicate::{Order, OrderTerm, Predicate};
use crate::render::Rendered;
use crate::source::{Cons, ContainsAll, Nil};
use core::marker::PhantomData;

/// A built SELECT query in dialect-agnostic form. Render with
/// [`SelectQuery::render`].
pub struct SelectQuery {
    /// FROM table.
    pub from: &'static str,
    /// JOIN clauses, in order.
    pub joins: Vec<Join>,
    /// Selected columns/expressions. Empty means `SELECT *`.
    pub columns: Vec<Expr>,
    /// WHERE predicate, if any.
    pub filter: Option<Expr>,
    /// ORDER BY terms, in order.
    pub order: Vec<OrderTerm>,
    /// LIMIT, if any.
    pub limit: Option<u64>,
    /// OFFSET, if any.
    pub offset: Option<u64>,
}

/// Fluent SELECT builder. `S` is the type-level set of in-scope entities. Start
/// with `E::query()`, which yields `Select<Cons<E, Nil>>`.
pub struct Select<S> {
    from: &'static str,
    joins: Vec<Join>,
    columns: Vec<Expr>,
    filter: Option<Expr>,
    order: Vec<OrderTerm>,
    limit: Option<u64>,
    offset: Option<u64>,
    _sources: PhantomData<fn() -> S>,
}

impl<S> Select<S> {
    /// Start a query over `from` with `S` as the initial source set. Called by
    /// [`Entity::query`].
    pub(crate) fn new(from: &'static str) -> Self {
        Select {
            from,
            joins: Vec::new(),
            columns: Vec::new(),
            filter: None,
            order: Vec::new(),
            limit: None,
            offset: None,
            _sources: PhantomData,
        }
    }

    /// Rebuild with a different source-set type after adding a join.
    fn with_join<S2>(mut self, join: Join) -> Select<S2> {
        self.joins.push(join);
        Select {
            from: self.from,
            joins: self.joins,
            columns: self.columns,
            filter: self.filter,
            order: self.order,
            limit: self.limit,
            offset: self.offset,
            _sources: PhantomData,
        }
    }

    /// Set the selected columns. Accepts a single [`Column`] or a tuple of them.
    ///
    /// Every referenced entity must be in scope (`S`), else a compile error.
    pub fn select<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: Selection,
        S: ContainsAll<Sel::Sources, Idxs>,
    {
        self.columns = selection.into_exprs();
        self
    }

    /// Add an `INNER JOIN` on the given entity with an ON condition, extending
    /// the in-scope set with `E2`.
    ///
    /// The joined entity is named via turbofish; the condition is usually an
    /// [`eq_column`](crate::Column::eq_column) between the two tables' keys:
    /// `.join::<Department>(User::department_id.eq_column(Department::id))`.
    ///
    /// The ON predicate is intentionally not checked against the source set: it
    /// references the entity being introduced, which is not yet in scope.
    pub fn join<E2: Entity>(self, on: impl OnClause) -> Select<Cons<E2, S>> {
        self.with_join(Join::new(JoinKind::Inner, E2::TABLE, on.into_on_expr()))
    }

    /// Add a `LEFT JOIN` on the given entity, extending the in-scope set.
    pub fn left_join<E2: Entity>(self, on: impl OnClause) -> Select<Cons<E2, S>> {
        self.with_join(Join::new(JoinKind::Left, E2::TABLE, on.into_on_expr()))
    }

    /// Add a WHERE predicate. Multiple calls are combined with `AND`.
    ///
    /// Every entity the predicate references must be in scope (`S`).
    pub fn filter<S2, Idxs>(mut self, predicate: Predicate<S2>) -> Self
    where
        S: ContainsAll<S2, Idxs>,
    {
        self.and_filter(predicate.into_expr());
        self
    }

    /// Add a WHERE predicate only if present. Combined with `AND`.
    ///
    /// Ergonomic for dynamic queries built from optional filter fields.
    pub fn filter_opt<S2, Idxs>(mut self, predicate: Option<Predicate<S2>>) -> Self
    where
        S: ContainsAll<S2, Idxs>,
    {
        if let Some(p) = predicate {
            self.and_filter(p.into_expr());
        }
        self
    }

    /// Append an ORDER BY term. The ordered entity must be in scope (`S`).
    pub fn order_by<S2, Idxs>(mut self, term: Order<S2>) -> Self
    where
        S: ContainsAll<S2, Idxs>,
    {
        self.order.push(term.into_term());
        self
    }

    /// Set LIMIT.
    pub fn limit(mut self, n: u64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Set OFFSET.
    pub fn offset(mut self, n: u64) -> Self {
        self.offset = Some(n);
        self
    }

    /// Finalize into a [`SelectQuery`] AST.
    pub fn build(self) -> SelectQuery {
        SelectQuery {
            from: self.from,
            joins: self.joins,
            columns: self.columns,
            filter: self.filter,
            order: self.order,
            limit: self.limit,
            offset: self.offset,
        }
    }

    /// Convenience: build and render in one step.
    pub fn render<D: Dialect>(self, dialect: &D) -> Rendered {
        self.build().render(dialect)
    }

    fn and_filter(&mut self, expr: Expr) {
        self.filter = Some(match self.filter.take() {
            Some(prev) => Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(prev),
                rhs: Box::new(expr),
            },
            None => expr,
        });
    }
}

/// A JOIN `ON` condition. Implemented for any [`Predicate`] regardless of the
/// entities it references: the ON clause names the entity being introduced,
/// which is not yet in scope, so its source set is intentionally not checked.
pub trait OnClause {
    /// Lower the condition into a source-erased AST expression.
    fn into_on_expr(self) -> Expr;
}

impl<S> OnClause for Predicate<S> {
    fn into_on_expr(self) -> Expr {
        self.into_expr()
    }
}

/// Column(s) usable in a SELECT list, carrying the entities they reference as a
/// type-level source set.
pub trait Selection {
    /// The set of entities this selection references.
    type Sources;
    /// Lower the selection into a list of AST expressions.
    fn into_exprs(self) -> Vec<Expr>;
}

impl<E, T> Selection for Column<E, T> {
    type Sources = Cons<E, Nil>;
    fn into_exprs(self) -> Vec<Expr> {
        vec![Expr::Column {
            table: self.table,
            name: self.name,
        }]
    }
}

/// Build a cons-list type from a list of entity idents.
macro_rules! cons_ty {
    () => { Nil };
    ($head:ident $(, $rest:ident)*) => { Cons<$head, cons_ty!($($rest),*)> };
}

macro_rules! selection_tuple {
    ($($e:ident $t:ident $idx:tt),+) => {
        impl<$($e, $t),+> Selection for ($(Column<$e, $t>,)+) {
            type Sources = cons_ty!($($e),+);
            fn into_exprs(self) -> Vec<Expr> {
                vec![$(Expr::Column { table: self.$idx.table, name: self.$idx.name }),+]
            }
        }
    };
}

selection_tuple!(Ea Ta 0, Eb Tb 1);
selection_tuple!(Ea Ta 0, Eb Tb 1, Ec Tc 2);
selection_tuple!(Ea Ta 0, Eb Tb 1, Ec Tc 2, Ed Td 3);
selection_tuple!(Ea Ta 0, Eb Tb 1, Ec Tc 2, Ed Td 3, Ee Te 4);
selection_tuple!(Ea Ta 0, Eb Tb 1, Ec Tc 2, Ed Td 3, Ee Te 4, Ef Tf 5);

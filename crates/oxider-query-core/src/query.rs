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
use crate::source::{Concat, Cons, ContainsAll, Nil};
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
    /// GROUP BY expressions, in order.
    pub group: Vec<Expr>,
    /// HAVING predicate, if any.
    pub having: Option<Expr>,
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
    group: Vec<Expr>,
    having: Option<Expr>,
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
            group: Vec::new(),
            having: None,
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
            group: self.group,
            having: self.having,
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

    /// Set the GROUP BY expressions. Accepts a single [`Column`] or a tuple.
    ///
    /// Every grouped entity must be in scope (`S`).
    pub fn group_by<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: Selection,
        S: ContainsAll<Sel::Sources, Idxs>,
    {
        self.group = selection.into_exprs();
        self
    }

    /// Add a HAVING predicate, filtering grouped rows. Multiple calls combine
    /// with `AND`. Typically built from aggregate comparisons, e.g.
    /// `count_all().gt(5)`.
    pub fn having<S2, Idxs>(mut self, predicate: Predicate<S2>) -> Self
    where
        S: ContainsAll<S2, Idxs>,
    {
        self.having = Some(match self.having.take() {
            Some(prev) => Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(prev),
                rhs: Box::new(predicate.into_expr()),
            },
            None => predicate.into_expr(),
        });
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
            group: self.group,
            having: self.having,
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

/// A single item usable in a SELECT or GROUP BY list: a [`Column`] or an
/// [`Aggregate`](crate::Aggregate). Carries the entities it references as a
/// type-level source set.
pub trait SelectItem {
    /// The set of entities this item references.
    type Sources;
    /// Lower the item into an AST expression.
    fn into_select_expr(self) -> Expr;
}

impl<E, T> SelectItem for Column<E, T> {
    type Sources = Cons<E, Nil>;
    fn into_select_expr(self) -> Expr {
        Expr::Column {
            table: self.table,
            name: self.name,
        }
    }
}

/// One or more items usable in a SELECT or GROUP BY list, carrying the union of
/// the entities they reference as a type-level source set.
pub trait Selection {
    /// The set of entities this selection references.
    type Sources;
    /// Lower the selection into a list of AST expressions.
    fn into_exprs(self) -> Vec<Expr>;
}

/// A single item is a selection of one.
impl<I: SelectItem> Selection for I {
    type Sources = I::Sources;
    fn into_exprs(self) -> Vec<Expr> {
        vec![self.into_select_expr()]
    }
}

// Tuple selections concatenate their elements' source sets (right-folded via
// `Concat`). Written out per arity because the nested associated-type bounds
// cannot be produced by a simple declarative macro.

impl<A: SelectItem, B: SelectItem> Selection for (A, B)
where
    A::Sources: Concat<B::Sources>,
{
    type Sources = <A::Sources as Concat<B::Sources>>::Out;
    fn into_exprs(self) -> Vec<Expr> {
        vec![self.0.into_select_expr(), self.1.into_select_expr()]
    }
}

impl<A: SelectItem, B: SelectItem, C: SelectItem> Selection for (A, B, C)
where
    B::Sources: Concat<C::Sources>,
    A::Sources: Concat<<B::Sources as Concat<C::Sources>>::Out>,
{
    type Sources = <A::Sources as Concat<<B::Sources as Concat<C::Sources>>::Out>>::Out;
    fn into_exprs(self) -> Vec<Expr> {
        vec![
            self.0.into_select_expr(),
            self.1.into_select_expr(),
            self.2.into_select_expr(),
        ]
    }
}

impl<A: SelectItem, B: SelectItem, C: SelectItem, D: SelectItem> Selection for (A, B, C, D)
where
    C::Sources: Concat<D::Sources>,
    B::Sources: Concat<<C::Sources as Concat<D::Sources>>::Out>,
    A::Sources: Concat<<B::Sources as Concat<<C::Sources as Concat<D::Sources>>::Out>>::Out>,
{
    type Sources = <A::Sources as Concat<
        <B::Sources as Concat<<C::Sources as Concat<D::Sources>>::Out>>::Out,
    >>::Out;
    fn into_exprs(self) -> Vec<Expr> {
        vec![
            self.0.into_select_expr(),
            self.1.into_select_expr(),
            self.2.into_select_expr(),
            self.3.into_select_expr(),
        ]
    }
}

impl<A: SelectItem, B: SelectItem, C: SelectItem, D: SelectItem, E: SelectItem> Selection
    for (A, B, C, D, E)
where
    D::Sources: Concat<E::Sources>,
    C::Sources: Concat<<D::Sources as Concat<E::Sources>>::Out>,
    B::Sources: Concat<<C::Sources as Concat<<D::Sources as Concat<E::Sources>>::Out>>::Out>,
    A::Sources: Concat<
        <B::Sources as Concat<
            <C::Sources as Concat<<D::Sources as Concat<E::Sources>>::Out>>::Out,
        >>::Out,
    >,
{
    type Sources = <A::Sources as Concat<
        <B::Sources as Concat<
            <C::Sources as Concat<<D::Sources as Concat<E::Sources>>::Out>>::Out,
        >>::Out,
    >>::Out;
    fn into_exprs(self) -> Vec<Expr> {
        vec![
            self.0.into_select_expr(),
            self.1.into_select_expr(),
            self.2.into_select_expr(),
            self.3.into_select_expr(),
            self.4.into_select_expr(),
        ]
    }
}

impl<A: SelectItem, B: SelectItem, C: SelectItem, D: SelectItem, E: SelectItem, F: SelectItem>
    Selection for (A, B, C, D, E, F)
where
    E::Sources: Concat<F::Sources>,
    D::Sources: Concat<<E::Sources as Concat<F::Sources>>::Out>,
    C::Sources: Concat<<D::Sources as Concat<<E::Sources as Concat<F::Sources>>::Out>>::Out>,
    B::Sources: Concat<
        <C::Sources as Concat<
            <D::Sources as Concat<<E::Sources as Concat<F::Sources>>::Out>>::Out,
        >>::Out,
    >,
    A::Sources: Concat<
        <B::Sources as Concat<
            <C::Sources as Concat<
                <D::Sources as Concat<<E::Sources as Concat<F::Sources>>::Out>>::Out,
            >>::Out,
        >>::Out,
    >,
{
    type Sources = <A::Sources as Concat<
        <B::Sources as Concat<
            <C::Sources as Concat<
                <D::Sources as Concat<<E::Sources as Concat<F::Sources>>::Out>>::Out,
            >>::Out,
        >>::Out,
    >>::Out;
    fn into_exprs(self) -> Vec<Expr> {
        vec![
            self.0.into_select_expr(),
            self.1.into_select_expr(),
            self.2.into_select_expr(),
            self.3.into_select_expr(),
            self.4.into_select_expr(),
            self.5.into_select_expr(),
        ]
    }
}

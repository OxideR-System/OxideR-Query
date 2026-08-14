//! The SELECT query builder and its dialect-agnostic output AST.

use crate::column::{Column, Entity};
use crate::dialect::Dialect;
use crate::expr::{BinOp, Expr};
use crate::join::{Join, JoinKind};
use crate::predicate::{OrderTerm, Predicate};
use crate::render::Rendered;
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

/// Fluent SELECT builder over entity `E`. Start with `E::query()`.
pub struct Select<E> {
    joins: Vec<Join>,
    columns: Vec<Expr>,
    filter: Option<Expr>,
    order: Vec<OrderTerm>,
    limit: Option<u64>,
    offset: Option<u64>,
    _marker: PhantomData<fn() -> E>,
}

impl<E> Select<E> {
    pub(crate) fn new() -> Self {
        Select {
            joins: Vec::new(),
            columns: Vec::new(),
            filter: None,
            order: Vec::new(),
            limit: None,
            offset: None,
            _marker: PhantomData,
        }
    }
}

impl<E: Entity> Select<E> {
    /// Set the selected columns. Accepts a single [`Column`] or a tuple of them.
    pub fn select<S: Selection>(mut self, selection: S) -> Self {
        self.columns = selection.into_exprs();
        self
    }

    /// Add an `INNER JOIN` on the given entity with an ON condition.
    ///
    /// The joined entity is named via turbofish; the condition is usually an
    /// [`eq_column`](crate::Column::eq_column) between the two tables' keys:
    /// `.join::<Department>(User::department_id.eq_column(Department::id))`.
    pub fn join<E2: Entity>(mut self, on: Predicate) -> Self {
        self.joins
            .push(Join::new(JoinKind::Inner, E2::TABLE, on.into_expr()));
        self
    }

    /// Add a `LEFT JOIN` on the given entity with an ON condition.
    pub fn left_join<E2: Entity>(mut self, on: Predicate) -> Self {
        self.joins
            .push(Join::new(JoinKind::Left, E2::TABLE, on.into_expr()));
        self
    }

    /// Add a WHERE predicate. Multiple calls are combined with `AND`.
    pub fn filter(mut self, predicate: Predicate) -> Self {
        self.and_filter(predicate.into_expr());
        self
    }

    /// Add a WHERE predicate only if present. Combined with `AND`.
    ///
    /// Ergonomic for dynamic queries built from optional filter fields.
    pub fn filter_opt(mut self, predicate: Option<Predicate>) -> Self {
        if let Some(p) = predicate {
            self.and_filter(p.into_expr());
        }
        self
    }

    /// Append an ORDER BY term.
    pub fn order_by(mut self, term: OrderTerm) -> Self {
        self.order.push(term);
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
            from: E::TABLE,
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

/// Column(s) usable in a SELECT list.
pub trait Selection {
    /// Lower the selection into a list of AST expressions.
    fn into_exprs(self) -> Vec<Expr>;
}

impl<E, T> Selection for Column<E, T> {
    fn into_exprs(self) -> Vec<Expr> {
        vec![Expr::Column {
            table: self.table,
            name: self.name,
        }]
    }
}

macro_rules! selection_tuple {
    ($($e:ident $t:ident $idx:tt),+) => {
        impl<$($e, $t),+> Selection for ($(Column<$e, $t>,)+) {
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

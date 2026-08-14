//! The SELECT query builder and its dialect-agnostic output AST.

use crate::column::{Column, Table};
use crate::dialect::Dialect;
use crate::expr::Expr;
use crate::expression::Expression;
use crate::render::Rendered;
use crate::sql_type::{Bool, SqlType};

/// A built SELECT query in dialect-agnostic form. Render it with
/// [`SelectQuery::render`].
#[derive(Debug, Clone, PartialEq)]
pub struct SelectQuery {
    /// Selected column/expression list. Empty means `SELECT *`.
    pub columns: Vec<Expr>,
    /// The FROM table, if any.
    pub from: Option<&'static str>,
    /// The WHERE predicate, if any.
    pub filter: Option<Expr>,
}

/// Fluent builder for SELECT queries. Start with [`Query::select`].
#[derive(Default)]
pub struct Select {
    columns: Vec<Expr>,
    from: Option<&'static str>,
    filter: Option<Expr>,
}

impl Select {
    /// Set the FROM table from a generated metamodel value.
    pub fn from<T: Table>(mut self, table: T) -> Self {
        self.from = Some(table.table_name());
        self
    }

    /// Set the selected columns. Accepts a single [`Column`] or a tuple of them.
    pub fn select<S: Selection>(mut self, selection: S) -> Self {
        self.columns = selection.into_exprs();
        self
    }

    /// Set the WHERE predicate. Only boolean-typed expressions are accepted.
    pub fn filter<P: Expression<Sql = Bool>>(mut self, pred: P) -> Self {
        self.filter = Some(pred.to_expr());
        self
    }

    /// Finalize into a [`SelectQuery`] AST.
    pub fn build(self) -> SelectQuery {
        SelectQuery {
            columns: self.columns,
            from: self.from,
            filter: self.filter,
        }
    }

    /// Convenience: build and render in one step.
    pub fn render<D: Dialect>(self, dialect: &D) -> Rendered {
        self.build().render(dialect)
    }
}

/// Entry point for building queries.
pub struct Query;

impl Query {
    /// Begin a SELECT query.
    pub fn select() -> Select {
        Select::default()
    }
}

/// Column(s) usable in a SELECT list.
pub trait Selection {
    /// Lower the selection into a list of AST expressions.
    fn into_exprs(self) -> Vec<Expr>;
}

impl<S: SqlType> Selection for Column<S> {
    fn into_exprs(self) -> Vec<Expr> {
        vec![self.to_expr()]
    }
}

macro_rules! selection_tuple {
    ($($name:ident),+) => {
        impl<$($name: SqlType),+> Selection for ($(Column<$name>,)+) {
            fn into_exprs(self) -> Vec<Expr> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                vec![$($name.to_expr()),+]
            }
        }
    };
}

selection_tuple!(A);
selection_tuple!(A, B);
selection_tuple!(A, B, C);
selection_tuple!(A, B, C, D);
selection_tuple!(A, B, C, D, E);
selection_tuple!(A, B, C, D, E, F);

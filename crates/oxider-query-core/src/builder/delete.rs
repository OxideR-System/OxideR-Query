//! The DELETE builder.
//!
//! Mirrors [`Update`](crate::builder::Update): the scope starts as the deleted
//! entity and grows with [`using`](Delete::using), which is how a delete
//! qualified by another table is expressed.

use crate::ast::dml::DeleteAst;
use crate::ast::node::{Node, TableRef};
use crate::ast::query::Source;
use crate::dialect::Dialect;
use crate::render::{render_delete, Bindings, RenderResult, Renderable, Rendered};
use crate::source::{Cons, ContainsAll, Nil};
use crate::typed::expr::Predicate;
use crate::typed::selection::SelectionIn;
use crate::typed::Table;
use core::marker::PhantomData;

/// A DELETE statement under construction.
pub struct Delete<E, S = Cons<E, Nil>> {
    ast: DeleteAst,
    bindings: Bindings,
    _marker: PhantomData<fn() -> (E, S)>,
}

impl<E, S> Delete<E, S> {
    /// Start a DELETE from a table.
    pub(crate) fn new(table: TableRef) -> Self {
        Delete {
            ast: DeleteAst {
                table,
                using: Vec::new(),
                filter: None,
                returning: Vec::new(),
            },
            bindings: Bindings::new(),
            _marker: PhantomData,
        }
    }

    fn retype<S2>(self) -> Delete<E, S2> {
        Delete {
            ast: self.ast,
            bindings: self.bindings,
            _marker: PhantomData,
        }
    }

    /// Bring another table into the statement (`DELETE ... USING`), so the
    /// condition may read from it.
    pub fn using<E2>(mut self, table: Table<E2>) -> Delete<E, Cons<E2, S>> {
        self.ast.using.push(Source::Table(table.reference()));
        self.retype()
    }

    /// Add a WHERE condition. Repeated calls are combined with `AND`.
    ///
    /// A delete with no condition removes every row, which is legal SQL and
    /// occasionally what is meant, so it is not refused here.
    pub fn filter<S2, I>(mut self, predicate: Predicate<S2>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        self.ast.filter = Some(Node::and_opt(self.ast.filter.take(), predicate.into_node()));
        self
    }

    /// Add a WHERE condition only when there is one.
    pub fn filter_opt<S2, I>(self, predicate: Option<Predicate<S2>>) -> Self
    where
        S: ContainsAll<S2, I>,
    {
        match predicate {
            Some(predicate) => self.filter(predicate),
            None => self,
        }
    }

    /// Return expressions from the deleted rows.
    pub fn returning<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<S, Idxs>,
    {
        selection.append_nodes(&mut self.ast.returning);
        self
    }

    /// The accumulated statement.
    pub fn into_ast(self) -> DeleteAst {
        self.ast
    }

    /// Render for a dialect.
    pub fn to_sql(&self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_delete(&self.ast, dialect, &self.bindings)
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

impl<E, S> Renderable for Delete<E, S> {
    fn render_with(self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_delete(&self.ast, dialect, &self.bindings)
    }
}

impl<E, S> Clone for Delete<E, S> {
    fn clone(&self) -> Self {
        Delete {
            ast: self.ast.clone(),
            bindings: self.bindings.clone(),
            _marker: PhantomData,
        }
    }
}

impl<E, S> core::fmt::Debug for Delete<E, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Delete").field(&self.ast).finish()
    }
}

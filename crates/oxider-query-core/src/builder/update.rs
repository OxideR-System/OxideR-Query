//! The UPDATE builder.
//!
//! The scope parameter `S` starts as the updated entity alone and grows with
//! [`from`](Update::from), so an assignment or a condition may only reference a
//! table the statement actually names - the same check the SELECT builder makes.

use crate::ast::dml::{Assignment, UpdateAst};
use crate::ast::node::{Node, TableRef};
use crate::ast::query::Source;
use crate::dialect::Dialect;
use crate::render::{render_update, Bindings, RenderResult, Renderable, Rendered};
use crate::source::{Cons, ContainsAll, Nil};
use crate::typed::expr::{IntoExpr, Predicate};
use crate::typed::selection::SelectionIn;
use crate::typed::{Column, Table};
use core::marker::PhantomData;

/// An UPDATE statement under construction.
pub struct Update<E, S = Cons<E, Nil>> {
    ast: UpdateAst,
    bindings: Bindings,
    _marker: PhantomData<fn() -> (E, S)>,
}

impl<E, S> Update<E, S> {
    /// Start an UPDATE of a table.
    pub(crate) fn new(table: TableRef) -> Self {
        Update {
            ast: UpdateAst {
                table,
                assignments: Vec::new(),
                from: Vec::new(),
                filter: None,
                returning: Vec::new(),
            },
            bindings: Bindings::new(),
            _marker: PhantomData,
        }
    }

    fn retype<S2>(self) -> Update<E, S2> {
        Update {
            ast: self.ast,
            bindings: self.bindings,
            _marker: PhantomData,
        }
    }

    /// Assign a column.
    ///
    /// The value is an expression, so `set(User::visits, User::visits.add(1))`
    /// increments in place rather than reading first.
    pub fn set<T, V, I>(mut self, column: Column<E, T>, value: V) -> Self
    where
        V: IntoExpr<T>,
        S: ContainsAll<V::Sources, I>,
    {
        self.ast.assignments.push(Assignment {
            column: column.name,
            value: value.into_expr_node(),
        });
        self
    }

    /// Set a column to NULL.
    ///
    /// Nullability is the schema's business rather than the column type's, so
    /// this is not restricted to columns the metamodel marks nullable; the
    /// database rejects the statement if the column forbids NULL.
    pub fn set_null<T>(mut self, column: Column<E, T>) -> Self {
        self.ast.assignments.push(Assignment {
            column: column.name,
            value: Node::Keyword("NULL"),
        });
        self
    }

    /// Bring another table into the statement (`UPDATE ... FROM`), so
    /// assignments and conditions may read from it.
    pub fn from<E2>(mut self, table: Table<E2>) -> Update<E, Cons<E2, S>> {
        self.ast.from.push(Source::Table(table.reference()));
        self.retype()
    }

    /// Add a WHERE condition. Repeated calls are combined with `AND`.
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

    /// Return expressions from the updated rows.
    pub fn returning<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<S, Idxs>,
    {
        selection.append_nodes(&mut self.ast.returning);
        self
    }

    /// The accumulated statement.
    pub fn into_ast(self) -> UpdateAst {
        self.ast
    }

    /// Render for a dialect.
    pub fn to_sql(&self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_update(&self.ast, dialect, &self.bindings)
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

impl<E, S> Renderable for Update<E, S> {
    fn render_with(self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_update(&self.ast, dialect, &self.bindings)
    }
}

impl<E, S> Clone for Update<E, S> {
    fn clone(&self) -> Self {
        Update {
            ast: self.ast.clone(),
            bindings: self.bindings.clone(),
            _marker: PhantomData,
        }
    }
}

impl<E, S> core::fmt::Debug for Update<E, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Update").field(&self.ast).finish()
    }
}

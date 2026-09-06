//! The INSERT builder, including multi-row inserts, `INSERT ... SELECT`, and
//! upserts.
//!
//! Two spellings, picked by how many rows there are:
//!
//! ```ignore
//! // One row, column by column. Reads well and needs no tuples.
//! User::insert().set(User::name, "ada").set(User::age, 36)
//!
//! // Several rows. The column list is fixed once, and every row is checked
//! // against it: wrong arity or wrong type is a compile error.
//! User::insert()
//!     .columns((User::name, User::age))
//!     .values(("ada", 36))
//!     .values(("grace", 45))
//! ```

use crate::ast::dml::{Assignment, ConflictAction, InsertAst, InsertSource, OnConflict};
use crate::ast::node::{Node, TableRef};
use crate::builder::select::Select;
use crate::dialect::Dialect;
use crate::render::{render_insert, RenderResult, Renderable, Rendered};
use crate::source::{Cons, Nil};
use crate::typed::expr::{Expr, IntoExpr};
use crate::typed::selection::SelectionIn;
use crate::typed::Column;
use core::marker::PhantomData;

/// An INSERT statement under construction.
///
/// `C` is the tuple of column types fixed by [`columns`](Insert::columns), and
/// `()` before that call.
pub struct Insert<E, C = ()> {
    ast: InsertAst,
    _marker: PhantomData<fn() -> (E, C)>,
}

impl<E, C> Insert<E, C> {
    /// Start an INSERT into a table.
    pub(crate) fn new(table: TableRef) -> Self {
        Insert {
            ast: InsertAst {
                table,
                columns: Vec::new(),
                source: InsertSource::Values(Vec::new()),
                on_conflict: None,
                returning: Vec::new(),
            },
            _marker: PhantomData,
        }
    }

    fn retype<C2>(self) -> Insert<E, C2> {
        Insert {
            ast: self.ast,
            _marker: PhantomData,
        }
    }

    /// The accumulated statement.
    pub fn into_ast(self) -> InsertAst {
        self.ast
    }

    /// Render for a dialect.
    pub fn to_sql(&self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_insert(&self.ast, dialect)
    }

    /// Take the inserted rows from a query instead of a `VALUES` list.
    ///
    /// The query's projection has to line up with the column list, which SQL
    /// checks and the types here do not: the projection is a runtime list of
    /// expressions, not a tuple.
    pub fn from_query<S, F>(mut self, query: Select<S, F>) -> Self {
        self.ast.source = InsertSource::Query(Box::new(query.into_ast()));
        self
    }

    /// Handle a uniqueness conflict on the given columns.
    ///
    /// An empty target means "any unique constraint", which is the only form
    /// MySQL understands.
    pub fn on_conflict(
        self,
        target: impl IntoIterator<Item = &'static str>,
    ) -> OnConflictTarget<E, C> {
        OnConflictTarget {
            insert: self,
            target: target.into_iter().collect(),
        }
    }

    /// Return expressions from the inserted rows.
    ///
    /// Native on PostgreSQL and SQLite; rejected at render time on MySQL, which
    /// has no equivalent.
    pub fn returning<Sel, Idxs>(mut self, selection: Sel) -> Self
    where
        Sel: SelectionIn<Cons<E, Nil>, Idxs>,
    {
        selection.append_nodes(&mut self.ast.returning);
        self
    }
}

impl<E> Insert<E> {
    /// Set one column of a single-row insert.
    ///
    /// Only available before [`columns`](Insert::columns), so the two spellings
    /// cannot be mixed into an inconsistent statement.
    pub fn set<T, V>(mut self, column: Column<E, T>, value: V) -> Self
    where
        V: IntoExpr<T, Sources = Nil>,
    {
        self.ast.columns.push(column.name);
        match &mut self.ast.source {
            InsertSource::Values(rows) => {
                if rows.is_empty() {
                    rows.push(Vec::new());
                }
                rows[0].push(value.into_expr_node());
            }
            // `set` after `from_query` would contradict the source; the column
            // still belongs in the list, which is what the query fills.
            InsertSource::Query(_) => {}
        }
        self
    }

    /// Fix the column list, switching to the multi-row spelling.
    pub fn columns<Cs>(mut self, columns: Cs) -> Insert<E, Cs::Types>
    where
        Cs: ColumnsOf<E>,
    {
        self.ast.columns = columns.names();
        self.ast.source = InsertSource::Values(Vec::new());
        self.retype()
    }
}

impl<E, C> Insert<E, C> {
    /// Append one row of values, checked against the column list.
    pub fn values<V>(mut self, values: V) -> Self
    where
        V: ValuesFor<C>,
    {
        if let InsertSource::Values(rows) = &mut self.ast.source {
            rows.push(values.into_nodes());
        }
        self
    }
}

impl<E, C> Renderable for Insert<E, C> {
    fn render_with(self, dialect: &dyn Dialect) -> RenderResult<Rendered> {
        render_insert(&self.ast, dialect)
    }
}

impl<E, C> Clone for Insert<E, C> {
    fn clone(&self) -> Self {
        Insert {
            ast: self.ast.clone(),
            _marker: PhantomData,
        }
    }
}

impl<E, C> core::fmt::Debug for Insert<E, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Insert").field(&self.ast).finish()
    }
}

/// A conflict clause waiting for its action.
pub struct OnConflictTarget<E, C> {
    insert: Insert<E, C>,
    target: Vec<&'static str>,
}

impl<E, C> OnConflictTarget<E, C> {
    /// Skip conflicting rows.
    ///
    /// Rejected on MySQL, which has no `DO NOTHING`: assign a column to itself
    /// with [`do_update`](OnConflictTarget::do_update) instead.
    pub fn do_nothing(mut self) -> Insert<E, C> {
        self.insert.ast.on_conflict = Some(OnConflict {
            target: self.target,
            action: ConflictAction::DoNothing,
        });
        self.insert
    }

    /// Update the conflicting row instead.
    pub fn do_update(self) -> ConflictUpdate<E, C> {
        ConflictUpdate {
            insert: self.insert,
            target: self.target,
            assignments: Vec::new(),
        }
    }
}

/// The `DO UPDATE SET ...` part of a conflict clause.
pub struct ConflictUpdate<E, C> {
    insert: Insert<E, C>,
    target: Vec<&'static str>,
    assignments: Vec<Assignment>,
}

impl<E, C> ConflictUpdate<E, C> {
    /// Assign a column of the conflicting row.
    ///
    /// Use [`excluded`] on the right to refer to the value that failed to
    /// insert.
    pub fn set<T, V>(mut self, column: Column<E, T>, value: V) -> Self
    where
        V: IntoExpr<T, Sources = Nil>,
    {
        self.assignments.push(Assignment {
            column: column.name,
            value: value.into_expr_node(),
        });
        self
    }

    /// Finish the conflict clause.
    pub fn end(mut self) -> Insert<E, C> {
        self.insert.ast.on_conflict = Some(OnConflict {
            target: self.target,
            action: ConflictAction::DoUpdate(self.assignments),
        });
        self.insert
    }
}

/// The value that failed to insert, for use in a conflict update.
///
/// Renders as `excluded.<column>` where the engine spells it that way and as
/// `VALUES(<column>)` on MySQL, so one upsert is portable.
pub fn excluded<E, T>(column: Column<E, T>) -> Expr<Nil, T> {
    Expr::new(Node::Excluded(column.name))
}

/// A tuple of columns of one entity, fixing an INSERT's column list.
pub trait ColumnsOf<E> {
    /// The tuple of Rust types the corresponding values must have.
    type Types;

    /// The column names, in order.
    fn names(&self) -> Vec<&'static str>;
}

/// A tuple of values matching a fixed column list.
pub trait ValuesFor<Types> {
    /// Lower each value into an AST node, in order.
    fn into_nodes(self) -> Vec<Node>;
}

/// Generate the `ColumnsOf`/`ValuesFor` impls for one tuple arity.
macro_rules! insert_tuple {
    ($($ty:ident $val:ident $field:tt),+) => {
        impl<E, $($ty),+> ColumnsOf<E> for ($(Column<E, $ty>,)+) {
            type Types = ($($ty,)+);
            fn names(&self) -> Vec<&'static str> {
                vec![$(self.$field.name),+]
            }
        }

        impl<$($ty, $val),+> ValuesFor<($($ty,)+)> for ($($val,)+)
        where
            $($val: IntoExpr<$ty, Sources = Nil>,)+
        {
            fn into_nodes(self) -> Vec<Node> {
                vec![$(self.$field.into_expr_node()),+]
            }
        }
    };
}

insert_tuple!(A VA 0);
insert_tuple!(A VA 0, B VB 1);
insert_tuple!(A VA 0, B VB 1, C VC 2);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5, G VG 6);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5, G VG 6, H VH 7);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5, G VG 6, H VH 7, I VI 8);
insert_tuple!(A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5, G VG 6, H VH 7, I VI 8, J VJ 9);
insert_tuple!(
    A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5, G VG 6, H VH 7, I VI 8, J VJ 9, K VK 10
);
insert_tuple!(
    A VA 0, B VB 1, C VC 2, D VD 3, E2 VE 4, F VF 5, G VG 6, H VH 7, I VI 8, J VJ 9, K VK 10,
    L VL 11
);

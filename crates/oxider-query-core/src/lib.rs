//! `oxider-query-core`: the dialect-agnostic, type-safe SQL query builder core.
//!
//! The crate is four layers, each of which can be read on its own:
//!
//! | Layer | Module | What it holds |
//! |-------|--------|---------------|
//! | AST | [`ast`] | Untyped statement and expression trees, plus the operator catalog |
//! | Dialect | [`dialect`] | Per-engine operator templates and structural capabilities |
//! | Render | [`render`] | One walker turning an AST into SQL text and bound parameters |
//! | Typed | [`typed`], [`builder`] | The API users write, and everything the compiler checks |
//!
//! The split is the same one QueryDSL makes between `Expression`/`Visitor` and
//! `SQLTemplates`, with one difference that matters: type safety here is a
//! property of the builder layer rather than of a generated class hierarchy, so
//! adding an operator is a line in a table rather than a method on six classes.
//!
//! There is no database dependency and no proc-macro here; `#[derive(Entity)]`
//! lives in `oxider-query-macros` and generates code against the types
//! re-exported below.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod ast;
pub mod builder;
pub mod dialect;
pub mod render;
pub mod source;
pub mod typed;
pub mod value;

pub use ast::{FrameBound, FrameExclusion, FrameUnit, Node, Operator};
pub use builder::{
    exists, not_exists, select_from, select_from_name, select_only, Delete, Insert, Select,
    Subquery, SubqueryOps, Update,
};
pub use dialect::{Caps, CastKind, Dialect, MySql, Postgres, Sqlite, Template};
pub use render::{Bindings, RenderError, RenderResult, Renderable, Rendered, MAX_DEPTH};
pub use source::{Cons, Nil};
pub use typed::{
    case_when, coalesce, col, count_all, curr_val, current_date, current_time, current_timestamp,
    greatest, least, next_val, null, nullif, param, raw, val, AggOps, Aggregate, Aliased, AnyExpr,
    BoolOps, Column, CompareOps, Entity, Expr, ExprExt, IntoExpr, MathOps, NumericAggOps, Only,
    Order, OrderOps, OrderedAggOps, Predicate, SelectionIn, Table, TemporalOps, TextOps, Window,
};
pub use value::{formats, Numeric, Orderable, SqlType, Temporal, ToSqlValue, Value};

/// Everything needed to write queries, in one import.
///
/// The operator traits have to be in scope for their methods to be callable, so
/// `use oxider_query_core::prelude::*;` is the intended way to start a module
/// that builds queries.
///
/// The builder types come along too. Naming one is unavoidable as soon as a
/// query leaves the expression where it was built: a helper that returns a
/// query needs `Select`, and an absent filter needs
/// `Option<Predicate<Only<E>>>` because there is nothing for inference to work
/// from.
///
/// [`Order`](crate::Order) and [`Table`](crate::Table) are deliberately left
/// out. Both are ordinary domain nouns that an application is likely to use as
/// an entity name, and neither has to be written down: `order_by` takes the
/// term directly, and `E::table()` produces the table. Import them from the
/// crate root in the rare case you need to name one.
pub mod prelude {
    pub use crate::ast::{FrameBound, FrameExclusion, FrameUnit};
    pub use crate::builder::{
        all_of, exists, not_exists, select_from, select_from_name, select_only, Delete, Insert,
        Select, Subquery, SubqueryOps, Update,
    };
    pub use crate::dialect::{CastKind, Dialect, MySql, Postgres, Sqlite};
    pub use crate::render::{RenderError, Renderable, Rendered};
    pub use crate::source::{Cons, Nil};
    pub use crate::typed::{
        bool_and, bool_or, case_when, coalesce, col, count_all, cume_dist, curr_val, current_date,
        current_time, current_timestamp, dense_rank, first_value, greatest, group_concat, lag,
        last_value, lead, least, next_val, nth_value, ntile, null, nullif, param, percent_rank,
        random, rank, raw, round_to, row_number, star, star_of, val, AggOps, Aggregate, Aliased,
        AnyExpr, BoolOps, Column, CompareOps, Entity, Expr, ExprExt, IntoExpr, MathOps,
        NumericAggOps, Only, OrderOps, OrderedAggOps, Predicate, SelectionIn, TemporalOps, TextOps,
        Window,
    };
    #[cfg(feature = "chrono")]
    pub use crate::typed::{now, today};
    pub use crate::value::{Numeric, Orderable, SqlType, Temporal, ToSqlValue, Value};
}

//! The typed layer: what the user writes, and what the compiler checks.
//!
//! Every type here is a thin, zero-cost wrapper over the untyped
//! [`Node`](crate::ast::node::Node) tree. Two type parameters recur:
//!
//! - `T`, the Rust type an expression evaluates to. It gates which operators
//!   are in scope, through the marker traits in [`crate::value`], and replaces
//!   QueryDSL's `NumberExpression` / `StringExpression` class hierarchy.
//! - `S`, the type-level set of entities an expression references. It is what
//!   turns "you forgot to join that table" from a runtime SQL error into a
//!   compile error, which QueryDSL cannot do at all.
//!
//! The operator families live in one module each so that adding an operator
//! touches one file rather than one enormous trait.

pub mod aggregate;
pub mod case;
pub mod column;
pub mod expr;
pub mod functions;
pub mod ops_bool;
pub mod ops_compare;
pub mod ops_datetime;
pub mod ops_math;
pub mod ops_text;
pub mod selection;
pub mod window;

pub use aggregate::{
    bool_and, bool_or, count_all, group_concat, percentile_cont, percentile_disc, AggOps,
    Aggregate, NumericAggOps, OrderedAggOps, PercentileCont, PercentileDisc,
};
pub use case::{case_when, CaseBuilder};
pub use column::{Aliased, Column, Entity, Table};
pub use expr::{Expr, IntoExpr, Only, Order, Predicate};
pub use functions::{
    coalesce, col, curr_val, current_date, current_time, current_timestamp, greatest, least,
    next_val, null, nullif, param, random, raw, round_to, star, star_of, val, Raw, VarArgs,
};
#[cfg(feature = "chrono")]
pub use functions::{now, today};
pub use ops_bool::BoolOps;
pub use ops_compare::{CompareOps, Merge, Merge3, OrderOps};
pub use ops_datetime::TemporalOps;
pub use ops_math::MathOps;
pub use ops_text::TextOps;
pub use selection::{AnyExpr, ExprExt, SelectionIn};
pub use window::{
    cume_dist, dense_rank, first_value, lag, last_value, lead, nth_value, ntile, percent_rank,
    rank, row_number, Framed, NoFrame, Window,
};

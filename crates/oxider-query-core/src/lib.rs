//! `oxider-query-core`: the dialect-agnostic, type-safe SQL query builder core.
//!
//! Holds the query metamodel ([`Entity`], [`Column`]), predicates and ordering
//! ([`Predicate`], [`Order`]), type-level source sets ([`source`]), the query
//! builder ([`Select`]), the dialect abstraction ([`Dialect`]) and the renderer.
//! It has no database dependency and no proc-macros; `#[derive(Entity)]` lives in
//! `oxider-query-macros` and generates code that references the types re-exported
//! here.
#![forbid(unsafe_code)]

pub mod column;
pub mod dialect;
pub mod expr;
pub mod join;
pub mod predicate;
pub mod query;
pub mod render;
pub mod source;
pub mod value;

pub use column::{Column, Entity};
pub use dialect::{Dialect, MySql, Postgres, Sqlite};
pub use expr::{BinOp, Expr};
pub use join::{Join, JoinKind};
pub use predicate::{Order, OrderDir, OrderTerm, Predicate};
pub use query::{OnClause, Select, SelectQuery, Selection};
pub use render::Rendered;
pub use source::{Cons, Nil};
pub use value::{Orderable, ToSqlValue, Value};

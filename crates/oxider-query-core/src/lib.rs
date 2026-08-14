//! `oxider-query-core`: the dialect-agnostic, type-safe SQL query builder core.
//!
//! This crate holds the query metamodel traits ([`Table`], [`Column`]), the
//! typed expression layer ([`Expression`], [`ExpressionMethods`]), the query
//! builder ([`Query`]/[`Select`]), the dialect abstraction ([`Dialect`]) and the
//! renderer. It has no database dependency and no proc-macros; the
//! `#[derive(Entity)]` macro lives in `oxider-query-macros` and generates code
//! that references the types re-exported here.
#![forbid(unsafe_code)]

pub mod column;
pub mod dialect;
pub mod expr;
pub mod expression;
pub mod query;
pub mod render;
pub mod sql_type;
pub mod value;

pub use column::{Column, Table};
pub use dialect::{Dialect, MySql, Postgres, Sqlite};
pub use expr::{BinOp, Expr};
pub use expression::{BoolExpr, Expression, ExpressionMethods, IntoExpr};
pub use query::{Query, Select, SelectQuery, Selection};
pub use render::Rendered;
pub use sql_type::{Bool, HasSqlType, Integer, Real, SqlType, Text};
pub use value::Value;

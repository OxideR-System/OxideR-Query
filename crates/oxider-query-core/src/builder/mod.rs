//! Statement builders.
//!
//! Each builder accumulates an AST while the type parameters track what is in
//! scope. Nothing here renders SQL itself; every builder hands its AST to
//! [`crate::render`], which is what lets one statement serve every dialect.

pub mod delete;
pub mod insert;
pub mod select;
pub mod subquery;
pub mod update;

pub use delete::Delete;
pub use insert::{
    excluded, AcceptsQuery, ColumnsOf, ConflictUpdate, FromQuery, Insert, NoRows, OnConflictTarget,
    OneRow, ValuesFor,
};
pub use select::{all_of, select_from, select_from_name, select_only, Locked, Select, Unlocked};
pub use subquery::{exists, not_exists, Subquery, SubqueryOps};
pub use update::Update;

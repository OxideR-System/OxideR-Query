//! The dialect-agnostic statement and expression AST.
//!
//! Everything here is plain data with no type parameters. The typed builders
//! enforce the compile-time rules and lower into these types; the renderer
//! turns them into SQL. Keeping the two apart is what lets one query serve
//! several dialects, and what would let a non-SQL backend reuse the same tree.

pub mod dml;
pub mod node;
pub mod operator;
pub mod query;
pub mod window;

pub use dml::{
    Assignment, ConflictAction, DeleteAst, InsertAst, InsertSource, OnConflict, UpdateAst,
};
pub use node::{ColumnRef, Node, RawPart, TableRef, WhenArm};
pub use operator::{Family, Operator};
pub use query::{
    Cte, Distinct, JoinAst, JoinKind, Lock, LockMode, LockWait, NullsOrder, OrderAst, OrderDir,
    SelectAst, SetOp, Source,
};
pub use window::{Frame, FrameBound, FrameExclusion, FrameUnit, InlineWindow, WindowSpec};

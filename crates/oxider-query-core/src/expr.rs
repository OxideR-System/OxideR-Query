//! Dialect-agnostic expression AST.
//!
//! This is the untyped intermediate representation. Type-safety lives in the
//! typed layer above ([`crate::expression`]); keeping the AST dynamic is what
//! lets a single renderer target multiple dialects without duplicating logic.

use crate::value::Value;

/// Binary operators supported by the expression AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `=`
    Eq,
    /// `<>`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `AND`
    And,
    /// `OR`
    Or,
}

impl BinOp {
    /// The SQL token for this operator (dialect-independent for the MVP set).
    pub fn as_sql(self) -> &'static str {
        match self {
            BinOp::Eq => "=",
            BinOp::Ne => "<>",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "AND",
            BinOp::Or => "OR",
        }
    }
}

/// An untyped expression node.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A qualified column reference (`"table"."name"`).
    Column {
        /// Owning table name.
        table: &'static str,
        /// Column name.
        name: &'static str,
    },
    /// A bound parameter carrying its value.
    Param(Value),
    /// A binary operation between two sub-expressions.
    Binary {
        /// The operator.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
}

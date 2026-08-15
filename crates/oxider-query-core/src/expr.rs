//! Dialect-agnostic expression AST.
//!
//! This is the untyped intermediate representation. Type-safety lives in the
//! typed column layer above ([`crate::column`]); keeping the AST dynamic is what
//! lets a single renderer target multiple dialects.

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
    /// `LIKE`
    Like,
    /// `AND`
    And,
    /// `OR`
    Or,
}

impl BinOp {
    /// The SQL token for this operator (dialect-independent for the current set).
    pub fn as_sql(self) -> &'static str {
        match self {
            BinOp::Eq => "=",
            BinOp::Ne => "<>",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Like => "LIKE",
            BinOp::And => "AND",
            BinOp::Or => "OR",
        }
    }
}

/// SQL aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggFunc {
    /// `COUNT`
    Count,
    /// `SUM`
    Sum,
    /// `AVG`
    Avg,
    /// `MIN`
    Min,
    /// `MAX`
    Max,
}

impl AggFunc {
    /// The SQL function name.
    pub fn as_sql(self) -> &'static str {
        match self {
            AggFunc::Count => "COUNT",
            AggFunc::Sum => "SUM",
            AggFunc::Avg => "AVG",
            AggFunc::Min => "MIN",
            AggFunc::Max => "MAX",
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
    /// An aggregate function call. `arg` is `None` for `COUNT(*)`.
    Aggregate {
        /// The aggregate function.
        func: AggFunc,
        /// The aggregated expression, or `None` for `COUNT(*)`.
        arg: Option<Box<Expr>>,
    },
}

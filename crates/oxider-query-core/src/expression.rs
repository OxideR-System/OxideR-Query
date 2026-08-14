//! Typed expression layer: the traits that enforce SQL-type-safe construction.
//!
//! Every typed expression knows its SQL type at compile time via
//! [`Expression::Sql`]. Comparison methods only accept a right-hand side of the
//! *same* SQL type, so mismatches (e.g. comparing a `Text` column to an integer)
//! fail to compile.

use crate::expr::{BinOp, Expr};
use crate::sql_type::{Bool, Integer, Real, SqlType, Text};
use crate::value::Value;

/// A typed SQL expression whose SQL type is known at compile time.
pub trait Expression {
    /// The SQL type this expression evaluates to.
    type Sql: SqlType;
    /// Lower this expression into the untyped AST.
    fn to_expr(&self) -> Expr;
}

/// Anything convertible into an expression of a given SQL type.
///
/// Implemented both for [`Expression`]s (via a blanket impl) and for Rust
/// literals, which become bound parameters. This is what lets `col.eq("x")`
/// accept a plain `&str` while still rejecting a wrongly-typed literal.
pub trait IntoExpr<Sql: SqlType> {
    /// Convert into the untyped AST for the target SQL type.
    fn into_expr(self) -> Expr;
}

// Any expression converts into an expression of its own SQL type.
// This does not overlap the literal impls below: those are for foreign types
// (`&str`, `String`, `i64`, ...) which cannot implement the local `Expression`
// trait, so the coherence checker proves the impls disjoint.
impl<S: SqlType, E: Expression<Sql = S>> IntoExpr<S> for E {
    fn into_expr(self) -> Expr {
        self.to_expr()
    }
}

impl IntoExpr<Integer> for i16 {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Int(self as i64))
    }
}
impl IntoExpr<Integer> for i32 {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Int(self as i64))
    }
}
impl IntoExpr<Integer> for i64 {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Int(self))
    }
}
impl IntoExpr<Text> for String {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Text(self))
    }
}
impl IntoExpr<Text> for &str {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Text(self.to_string()))
    }
}
impl IntoExpr<Bool> for bool {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Bool(self))
    }
}
impl IntoExpr<Real> for f32 {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Real(self as f64))
    }
}
impl IntoExpr<Real> for f64 {
    fn into_expr(self) -> Expr {
        Expr::Param(Value::Real(self))
    }
}

/// Fluent comparison and boolean operators available on every typed expression.
///
/// Blanket-implemented for all [`Expression`]s, so columns and predicates get
/// these methods for free. Each comparison requires the right-hand side to share
/// this expression's SQL type, enforcing type-safety at the call site.
pub trait ExpressionMethods: Expression + Sized {
    /// `self = rhs`
    fn eq<R: IntoExpr<Self::Sql>>(self, rhs: R) -> BoolExpr {
        self.binop(BinOp::Eq, rhs)
    }
    /// `self <> rhs`
    fn ne<R: IntoExpr<Self::Sql>>(self, rhs: R) -> BoolExpr {
        self.binop(BinOp::Ne, rhs)
    }
    /// `self < rhs`
    fn lt<R: IntoExpr<Self::Sql>>(self, rhs: R) -> BoolExpr {
        self.binop(BinOp::Lt, rhs)
    }
    /// `self <= rhs`
    fn le<R: IntoExpr<Self::Sql>>(self, rhs: R) -> BoolExpr {
        self.binop(BinOp::Le, rhs)
    }
    /// `self > rhs`
    fn gt<R: IntoExpr<Self::Sql>>(self, rhs: R) -> BoolExpr {
        self.binop(BinOp::Gt, rhs)
    }
    /// `self >= rhs`
    fn ge<R: IntoExpr<Self::Sql>>(self, rhs: R) -> BoolExpr {
        self.binop(BinOp::Ge, rhs)
    }

    /// Build a binary predicate from this expression and a typed right-hand side.
    fn binop<R: IntoExpr<Self::Sql>>(self, op: BinOp, rhs: R) -> BoolExpr {
        BoolExpr(Expr::Binary {
            op,
            lhs: Box::new(self.to_expr()),
            rhs: Box::new(rhs.into_expr()),
        })
    }
}

impl<T: Expression> ExpressionMethods for T {}

/// A boolean predicate expression (SQL type [`Bool`]).
///
/// Produced by the comparison methods; combine predicates with [`BoolExpr::and`]
/// and [`BoolExpr::or`].
pub struct BoolExpr(pub(crate) Expr);

impl Expression for BoolExpr {
    type Sql = Bool;
    fn to_expr(&self) -> Expr {
        self.0.clone()
    }
}

impl BoolExpr {
    /// Combine with another predicate using `AND`.
    pub fn and(self, other: BoolExpr) -> BoolExpr {
        BoolExpr(Expr::Binary {
            op: BinOp::And,
            lhs: Box::new(self.0),
            rhs: Box::new(other.0),
        })
    }
    /// Combine with another predicate using `OR`.
    pub fn or(self, other: BoolExpr) -> BoolExpr {
        BoolExpr(Expr::Binary {
            op: BinOp::Or,
            lhs: Box::new(self.0),
            rhs: Box::new(other.0),
        })
    }
}

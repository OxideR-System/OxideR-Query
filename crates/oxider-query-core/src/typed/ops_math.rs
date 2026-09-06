//! Arithmetic and math functions, available on numeric expressions.
//!
//! Arithmetic keeps the operand's Rust type, so `price.add(10)` is still a
//! number of the same kind. Functions that are mathematically real-valued
//! (`sqrt`, `ln`, the trigonometric family) return `f64` regardless of the
//! input type, which is what every supported engine actually returns.

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::source::Concat;
use crate::typed::expr::{Expr, IntoExpr};
use crate::typed::ops_compare::Merge;
use crate::value::{Numeric, SqlType};

/// Build an arithmetic method that keeps the operand type.
macro_rules! arithmetic {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name<R>(self, rhs: R) -> Expr<Merge<Self::Sources, R::Sources>, T>
        where
            R: IntoExpr<T>,
            Self::Sources: Concat<R::Sources>,
        {
            Expr::new(Node::binary(
                $op,
                self.into_expr_node(),
                rhs.into_expr_node(),
            ))
        }
    };
}

/// Build a one-argument function that returns `f64`.
macro_rules! real_fn {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name(self) -> Expr<Self::Sources, f64> {
            Expr::new(Node::unary($op, self.into_expr_node()))
        }
    };
}

/// Build a one-argument function that keeps the operand type.
macro_rules! same_type_fn {
    ($name:ident, $op:expr, $doc:literal) => {
        #[doc = $doc]
        fn $name(self) -> Expr<Self::Sources, T> {
            Expr::new(Node::unary($op, self.into_expr_node()))
        }
    };
}

/// Arithmetic and math operations on a numeric expression.
pub trait MathOps<T>: IntoExpr<T> + Sized
where
    T: SqlType + Numeric,
{
    arithmetic!(add, Operator::Add, "`self + rhs`");
    arithmetic!(sub, Operator::Sub, "`self - rhs`");
    arithmetic!(mul, Operator::Mul, "`self * rhs`");
    arithmetic!(div, Operator::Div, "`self / rhs`");
    arithmetic!(rem, Operator::Mod, "`self % rhs`, the remainder");

    same_type_fn!(abs, Operator::Abs, "`ABS(self)`");
    same_type_fn!(ceil, Operator::Ceil, "Round up to an integral value");
    same_type_fn!(floor, Operator::Floor, "Round down to an integral value");
    same_type_fn!(
        round,
        Operator::Round,
        "Round to the nearest integral value"
    );
    same_type_fn!(negate, Operator::Negate, "`-self`");
    same_type_fn!(sign, Operator::Sign, "-1, 0, or 1 by sign");

    real_fn!(sqrt, Operator::Sqrt, "`SQRT(self)`");
    real_fn!(exp, Operator::Exp, "`EXP(self)`, e raised to self");
    real_fn!(ln, Operator::Ln, "Natural logarithm");
    real_fn!(sin, Operator::Sin, "`SIN(self)`");
    real_fn!(cos, Operator::Cos, "`COS(self)`");
    real_fn!(tan, Operator::Tan, "`TAN(self)`");
    real_fn!(asin, Operator::Asin, "`ASIN(self)`");
    real_fn!(acos, Operator::Acos, "`ACOS(self)`");
    real_fn!(atan, Operator::Atan, "`ATAN(self)`");
    real_fn!(sinh, Operator::Sinh, "Hyperbolic sine");
    real_fn!(cosh, Operator::Cosh, "Hyperbolic cosine");
    real_fn!(tanh, Operator::Tanh, "Hyperbolic tangent");
    real_fn!(cot, Operator::Cot, "Cotangent");
    real_fn!(coth, Operator::Coth, "Hyperbolic cotangent");
    real_fn!(degrees, Operator::Degrees, "Radians converted to degrees");
    real_fn!(radians, Operator::Radians, "Degrees converted to radians");

    /// Round to `digits` decimal places.
    fn round_to(self, digits: i64) -> Expr<Self::Sources, T> {
        Expr::new(Node::op(
            Operator::RoundTo,
            [
                self.into_expr_node(),
                Node::Param(crate::value::Value::Int(digits)),
            ],
        ))
    }

    /// `POWER(self, exponent)`.
    fn power<R>(self, exponent: R) -> Expr<Merge<Self::Sources, R::Sources>, f64>
    where
        R: IntoExpr<T>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::Power,
            self.into_expr_node(),
            exponent.into_expr_node(),
        ))
    }

    /// Logarithm of `self` in the given base.
    fn log<R>(self, base: R) -> Expr<Merge<Self::Sources, R::Sources>, f64>
    where
        R: IntoExpr<T>,
        Self::Sources: Concat<R::Sources>,
    {
        // The template spells this `LOG(base, value)`, so the base leads.
        Expr::new(Node::binary(
            Operator::Log,
            base.into_expr_node(),
            self.into_expr_node(),
        ))
    }
}

impl<E, T: SqlType + Numeric> MathOps<T> for crate::typed::Column<E, T> {}
impl<S, T: SqlType + Numeric> MathOps<T> for Expr<S, T> {}

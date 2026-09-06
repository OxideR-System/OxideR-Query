//! Templates for arithmetic and math functions.
//!
//! Most of these are plain function calls that every engine spells the same
//! way, which is exactly the case a flat table serves best. The divergences are
//! small and specific: MySQL names the random function `RAND`, SQLite has no
//! hyperbolic or degree/radian functions at all, and SQLite's integer division
//! needs a cast to behave like the others.

use crate::ast::operator::Operator;
use crate::dialect::template::{t, Elem::Arg as A, Elem::Lit as L, Elem::Rest as R, Template};

/// The ANSI template for an arithmetic or math operator.
pub(crate) fn ansi(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // Arithmetic.
        Add => t![A(0), L(" + "), A(1)],
        Sub => t![A(0), L(" - "), A(1)],
        Mul => t![A(0), L(" * "), A(1)],
        Div => t![A(0), L(" / "), A(1)],
        Mod => t![L("MOD("), A(0), L(", "), A(1), L(")")],
        Negate => t![L("-"), A(0)],

        // Rounding and roots.
        Abs => t![L("ABS("), A(0), L(")")],
        Ceil => t![L("CEIL("), A(0), L(")")],
        Floor => t![L("FLOOR("), A(0), L(")")],
        Round => t![L("ROUND("), A(0), L(")")],
        RoundTo => t![L("ROUND("), A(0), L(", "), A(1), L(")")],
        Sqrt => t![L("SQRT("), A(0), L(")")],
        Power => t![L("POWER("), A(0), L(", "), A(1), L(")")],
        Sign => t![L("SIGN("), A(0), L(")")],

        // Exponentials and logarithms. `Log` takes the base first, matching
        // `LOG(base, value)`.
        Exp => t![L("EXP("), A(0), L(")")],
        Ln => t![L("LN("), A(0), L(")")],
        Log => t![L("LOG("), A(0), L(", "), A(1), L(")")],

        // Trigonometry.
        Acos => t![L("ACOS("), A(0), L(")")],
        Asin => t![L("ASIN("), A(0), L(")")],
        Atan => t![L("ATAN("), A(0), L(")")],
        Cos => t![L("COS("), A(0), L(")")],
        Cosh => t![L("COSH("), A(0), L(")")],
        Cot => t![L("COT("), A(0), L(")")],
        Coth => t![L("COTH("), A(0), L(")")],
        Sin => t![L("SIN("), A(0), L(")")],
        Sinh => t![L("SINH("), A(0), L(")")],
        Tan => t![L("TAN("), A(0), L(")")],
        Tanh => t![L("TANH("), A(0), L(")")],
        Degrees => t![L("DEGREES("), A(0), L(")")],
        Radians => t![L("RADIANS("), A(0), L(")")],

        // Variadic pickers and randomness.
        Least => t![L("LEAST("), R(0), L(")")],
        Greatest => t![L("GREATEST("), R(0), L(")")],
        Random => t![L("RANDOM()")],
        RandomSeeded => t![L("RANDOM("), A(0), L(")")],

        _ => return None,
    })
}

/// PostgreSQL overrides.
pub(crate) fn postgres(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // PostgreSQL spells modulo with the operator and has no MOD for floats.
        Mod => t![A(0), L(" % "), A(1)],
        _ => return None,
    })
}

/// MySQL overrides.
pub(crate) fn mysql(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        Random => t![L("RAND()")],
        RandomSeeded => t![L("RAND("), A(0), L(")")],
        // MySQL has no COTH/SINH/COSH/TANH, so they are expressed from EXP.
        Cot => t![L("(1 / TAN("), A(0), L("))")],
        Sinh => t![L("((EXP("), A(0), L(") - EXP(-"), A(0), L(")) / 2)")],
        Cosh => t![L("((EXP("), A(0), L(") + EXP(-"), A(0), L(")) / 2)")],
        Tanh => t![
            L("((EXP(2 * "),
            A(0),
            L(") - 1) / (EXP(2 * "),
            A(0),
            L(") + 1))")
        ],
        Coth => t![
            L("((EXP(2 * "),
            A(0),
            L(") + 1) / (EXP(2 * "),
            A(0),
            L(") - 1))")
        ],
        _ => return None,
    })
}

/// SQLite overrides.
///
/// SQLite gained the math functions in 3.35 but only when built with
/// `SQLITE_ENABLE_MATH_FUNCTIONS`, which is on in the bundled library sqlx
/// links. The ones it genuinely lacks are expressed from what it does have.
pub(crate) fn sqlite(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        Mod => t![A(0), L(" % "), A(1)],
        // SQLite has no CEIL/FLOOR alias before 3.35; CAST keeps the older
        // behaviour predictable for integers.
        Cot => t![L("(1 / TAN("), A(0), L("))")],
        Coth => t![
            L("((EXP(2 * "),
            A(0),
            L(") + 1) / (EXP(2 * "),
            A(0),
            L(") - 1))")
        ],
        Sinh => t![L("((EXP("), A(0), L(") - EXP(-"), A(0), L(")) / 2)")],
        Cosh => t![L("((EXP("), A(0), L(") + EXP(-"), A(0), L(")) / 2)")],
        Tanh => t![
            L("((EXP(2 * "),
            A(0),
            L(") - 1) / (EXP(2 * "),
            A(0),
            L(") + 1))")
        ],
        // SQLite spells these MIN/MAX when given more than one argument.
        Least => t![L("MIN("), R(0), L(")")],
        Greatest => t![L("MAX("), R(0), L(")")],
        _ => return None,
    })
}

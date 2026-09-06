//! Templates for comparison, boolean logic, and conditional operators.
//!
//! These are the operators every dialect agrees on almost completely, so the
//! ANSI table carries nearly all of them and the per-dialect functions are
//! nearly empty. The exceptions are worth naming: MySQL has a real `XOR`
//! keyword, and neither MySQL nor SQLite (before 3.39) has `IS DISTINCT FROM`.

use crate::ast::operator::Operator;
use crate::dialect::template::{t, Elem::Arg as A, Elem::Lit as L, Elem::Rest as R, Template};

/// The ANSI template for a comparison, boolean, or conditional operator.
pub(crate) fn ansi(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // Comparison.
        Eq => t![A(0), L(" = "), A(1)],
        Ne => t![A(0), L(" <> "), A(1)],
        Lt => t![A(0), L(" < "), A(1)],
        Le => t![A(0), L(" <= "), A(1)],
        Gt => t![A(0), L(" > "), A(1)],
        Ge => t![A(0), L(" >= "), A(1)],
        Between => t![A(0), L(" BETWEEN "), A(1), L(" AND "), A(2)],
        NotBetween => t![A(0), L(" NOT BETWEEN "), A(1), L(" AND "), A(2)],
        IsNull => t![A(0), L(" IS NULL")],
        IsNotNull => t![A(0), L(" IS NOT NULL")],
        In => t![A(0), L(" IN "), A(1)],
        NotIn => t![A(0), L(" NOT IN "), A(1)],
        IsDistinctFrom => t![A(0), L(" IS DISTINCT FROM "), A(1)],
        IsNotDistinctFrom => t![A(0), L(" IS NOT DISTINCT FROM "), A(1)],

        // Quantifiers and existence. The argument is a subquery node, which
        // renders with its own parentheses.
        Any => t![L("ANY "), A(0)],
        All => t![L("ALL "), A(0)],
        Exists => t![L("EXISTS "), A(0)],
        NotExists => t![L("NOT EXISTS "), A(0)],

        // Boolean logic. ANSI has no XOR, so inequality of two booleans stands
        // in for it; MySQL overrides with its native keyword.
        And => t![A(0), L(" AND "), A(1)],
        Or => t![A(0), L(" OR "), A(1)],
        Not => t![L("NOT "), A(0)],
        Xor => t![A(0), L(" <> "), A(1)],

        // Conditional. Casts are a dedicated AST node rather than an operator,
        // because the target type name is resolved from the dialect.
        Coalesce => t![L("COALESCE("), R(0), L(")")],
        NullIf => t![L("NULLIF("), A(0), L(", "), A(1), L(")")],

        _ => return None,
    })
}

/// PostgreSQL overrides: none, PostgreSQL is the ANSI baseline here.
pub(crate) fn postgres(_op: Operator) -> Option<Template> {
    None
}

/// MySQL overrides.
pub(crate) fn mysql(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // MySQL has a real XOR keyword.
        Xor => t![A(0), L(" XOR "), A(1)],
        // MySQL spells null-safe equality `<=>` and has no IS DISTINCT FROM.
        IsNotDistinctFrom => t![A(0), L(" <=> "), A(1)],
        IsDistinctFrom => t![L("NOT ("), A(0), L(" <=> "), A(1), L(")")],
        _ => return None,
    })
}

/// SQLite overrides.
pub(crate) fn sqlite(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // SQLite spells these IS / IS NOT, which are null-safe there.
        IsNotDistinctFrom => t![A(0), L(" IS "), A(1)],
        IsDistinctFrom => t![A(0), L(" IS NOT "), A(1)],
        _ => return None,
    })
}

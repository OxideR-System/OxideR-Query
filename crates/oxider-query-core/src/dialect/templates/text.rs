//! Templates for string functions and pattern matching.
//!
//! This is the family with the most real divergence. Concatenation is an
//! operator everywhere except MySQL, where it is a function. Substring and
//! position have both a standard `FROM ... FOR` spelling and a positional one.
//! Case-insensitive matching is a native operator on PostgreSQL (`ILIKE`) and
//! an emulation everywhere else. Regular expressions are not standard at all.

use crate::ast::operator::Operator;
use crate::dialect::template::{t, Elem::Arg as A, Elem::Lit as L, Template};

/// The ANSI template for a string operator.
pub(crate) fn ansi(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        Concat => t![A(0), L(" || "), A(1)],
        Lower => t![L("LOWER("), A(0), L(")")],
        Upper => t![L("UPPER("), A(0), L(")")],
        Trim => t![L("TRIM("), A(0), L(")")],
        LTrim => t![L("LTRIM("), A(0), L(")")],
        RTrim => t![L("RTRIM("), A(0), L(")")],
        Length => t![L("LENGTH("), A(0), L(")")],
        Substr => t![L("SUBSTRING("), A(0), L(" FROM "), A(1), L(")")],
        SubstrLen => t![
            L("SUBSTRING("),
            A(0),
            L(" FROM "),
            A(1),
            L(" FOR "),
            A(2),
            L(")")
        ],
        // POSITION takes the needle first, the haystack second, which is the
        // reverse of the argument order the builder uses.
        IndexOf => t![L("POSITION("), A(1), L(" IN "), A(0), L(")")],
        IndexOfFrom => t![
            L("(POSITION("),
            A(1),
            L(" IN SUBSTRING("),
            A(0),
            L(" FROM "),
            A(2),
            L(")) + "),
            A(2),
            L(" - 1)")
        ],
        Left => t![L("LEFT("), A(0), L(", "), A(1), L(")")],
        Right => t![L("RIGHT("), A(0), L(", "), A(1), L(")")],
        LPad => t![L("LPAD("), A(0), L(", "), A(1), L(")")],
        LPadFill => t![L("LPAD("), A(0), L(", "), A(1), L(", "), A(2), L(")")],
        RPad => t![L("RPAD("), A(0), L(", "), A(1), L(")")],
        RPadFill => t![L("RPAD("), A(0), L(", "), A(1), L(", "), A(2), L(")")],
        Replace => t![L("REPLACE("), A(0), L(", "), A(1), L(", "), A(2), L(")")],

        // Pattern matching. Every `contains`/`starts_with`/`ends_with` the
        // typed layer offers lowers to LikeEscape, because the pattern it
        // builds escapes the user's wildcards.
        Like => t![A(0), L(" LIKE "), A(1)],
        LikeEscape => t![A(0), L(" LIKE "), A(1), L(" ESCAPE "), A(2)],
        LikeIc => t![L("LOWER("), A(0), L(") LIKE LOWER("), A(1), L(")")],
        LikeEscapeIc => t![
            L("LOWER("),
            A(0),
            L(") LIKE LOWER("),
            A(1),
            L(") ESCAPE "),
            A(2)
        ],
        EqIgnoreCase => t![L("LOWER("), A(0), L(") = LOWER("), A(1), L(")")],
        StringIsEmpty => t![A(0), L(" = ''")],

        // No standard regular-expression operator exists; the PostgreSQL
        // spelling is the baseline and other dialects override it.
        Matches => t![A(0), L(" ~ "), A(1)],
        MatchesIc => t![A(0), L(" ~* "), A(1)],

        _ => return None,
    })
}

/// PostgreSQL overrides.
pub(crate) fn postgres(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // Native case-insensitive LIKE.
        LikeIc => t![A(0), L(" ILIKE "), A(1)],
        LikeEscapeIc => t![A(0), L(" ILIKE "), A(1), L(" ESCAPE "), A(2)],
        _ => return None,
    })
}

/// MySQL overrides.
pub(crate) fn mysql(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // `||` is logical OR in MySQL's default mode, so concatenation is a
        // function call.
        Concat => t![L("CONCAT("), A(0), L(", "), A(1), L(")")],
        Substr => t![L("SUBSTRING("), A(0), L(", "), A(1), L(")")],
        SubstrLen => t![L("SUBSTRING("), A(0), L(", "), A(1), L(", "), A(2), L(")")],
        IndexOf => t![L("LOCATE("), A(1), L(", "), A(0), L(")")],
        IndexOfFrom => t![L("LOCATE("), A(1), L(", "), A(0), L(", "), A(2), L(")")],
        Matches => t![A(0), L(" REGEXP BINARY "), A(1)],
        MatchesIc => t![A(0), L(" REGEXP "), A(1)],
        _ => return None,
    })
}

/// SQLite overrides.
///
/// SQLite has no `LEFT`/`RIGHT`/`LPAD`/`RPAD`/`POSITION`, so those are built
/// from `SUBSTR`, `INSTR` and `REPLACE`, and `REGEXP` only works when the host
/// registers the function.
pub(crate) fn sqlite(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        Substr => t![L("SUBSTR("), A(0), L(", "), A(1), L(")")],
        SubstrLen => t![L("SUBSTR("), A(0), L(", "), A(1), L(", "), A(2), L(")")],
        IndexOf => t![L("INSTR("), A(0), L(", "), A(1), L(")")],
        IndexOfFrom => t![
            L("(INSTR(SUBSTR("),
            A(0),
            L(", "),
            A(2),
            L("), "),
            A(1),
            L(") + "),
            A(2),
            L(" - 1)")
        ],
        Left => t![L("SUBSTR("), A(0), L(", 1, "), A(1), L(")")],
        Right => t![L("SUBSTR("), A(0), L(", -"), A(1), L(")")],
        Matches => t![A(0), L(" REGEXP "), A(1)],
        MatchesIc => t![L("LOWER("), A(0), L(") REGEXP LOWER("), A(1), L(")")],
        _ => return None,
    })
}

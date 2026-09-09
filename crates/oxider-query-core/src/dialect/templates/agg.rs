//! Templates for aggregate, window/ranking, and sequence functions.
//!
//! `DISTINCT` is not part of these templates. The renderer prefixes the first
//! argument with it when the aggregate node carries the flag, which lands it in
//! the one place SQL accepts it (`COUNT(DISTINCT x)`) without needing a second
//! template per function.
//!
//! `FILTER (WHERE ...)` is likewise applied by the renderer, which rewrites it
//! into a `CASE` inside the aggregate on engines that lack the clause.
//!
//! Nor is the sort. An ordinary aggregate carries it inside the parentheses and
//! an ordered-set aggregate carries it after them, and that difference follows
//! from the operator, so the renderer decides it rather than the table.

use crate::ast::operator::Operator;
use crate::dialect::template::{
    t, Elem::Arg as A, Elem::Ident as I, Elem::Lit as L, Elem::Rest as R, Elem::TextLiteral as S,
    Template,
};

/// The ANSI template for an aggregate, window, or sequence operator.
pub(crate) fn ansi(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // Aggregates.
        CountAll => t![L("COUNT(*)")],
        Count => t![L("COUNT("), A(0), L(")")],
        Sum => t![L("SUM("), A(0), L(")")],
        Avg => t![L("AVG("), A(0), L(")")],
        Min => t![L("MIN("), A(0), L(")")],
        Max => t![L("MAX("), A(0), L(")")],
        StdDev => t![L("STDDEV("), A(0), L(")")],
        StdDevPop => t![L("STDDEV_POP("), A(0), L(")")],
        StdDevSamp => t![L("STDDEV_SAMP("), A(0), L(")")],
        Variance => t![L("VARIANCE("), A(0), L(")")],
        VarPop => t![L("VAR_POP("), A(0), L(")")],
        VarSamp => t![L("VAR_SAMP("), A(0), L(")")],
        BoolAnd => t![L("BOOL_AND("), A(0), L(")")],
        BoolOr => t![L("BOOL_OR("), A(0), L(")")],
        GroupConcat => t![L("STRING_AGG("), A(0), L(", "), A(1), L(")")],
        Corr => t![L("CORR("), A(0), L(", "), A(1), L(")")],
        CovarPop => t![L("COVAR_POP("), A(0), L(", "), A(1), L(")")],
        CovarSamp => t![L("COVAR_SAMP("), A(0), L(", "), A(1), L(")")],
        // Ordered-set aggregates. The template covers the call; the renderer
        // appends `WITHIN GROUP (ORDER BY ...)` from the node's sort terms,
        // which is the same split that keeps DISTINCT and FILTER out of here.
        PercentileCont => t![L("PERCENTILE_CONT("), A(0), L(")")],
        PercentileDisc => t![L("PERCENTILE_DISC("), A(0), L(")")],

        // Window and ranking functions.
        RowNumber => t![L("ROW_NUMBER()")],
        Rank => t![L("RANK()")],
        DenseRank => t![L("DENSE_RANK()")],
        PercentRank => t![L("PERCENT_RANK()")],
        CumeDist => t![L("CUME_DIST()")],
        Ntile => t![L("NTILE("), A(0), L(")")],
        Lag => t![L("LAG("), R(0), L(")")],
        Lead => t![L("LEAD("), R(0), L(")")],
        FirstValue => t![L("FIRST_VALUE("), A(0), L(")")],
        LastValue => t![L("LAST_VALUE("), A(0), L(")")],
        NthValue => t![L("NTH_VALUE("), A(0), L(", "), A(1), L(")")],
        // Sequences. The name is an identifier rather than a bound parameter,
        // because engines take it as a name rather than a value - so it is
        // quoted as one, not spliced.
        NextVal => t![L("NEXT VALUE FOR "), I(0)],
        CurrVal => return None,

        _ => return None,
    })
}

/// PostgreSQL overrides.
pub(crate) fn postgres(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        // PostgreSQL names the sequence with a string, so the name is rendered
        // as an escaped literal rather than pasted between two quote marks.
        NextVal => t![L("NEXTVAL("), S(0), L(")")],
        CurrVal => t![L("CURRVAL("), S(0), L(")")],
        _ => return None,
    })
}

/// MySQL overrides.
///
/// MySQL has no `BOOL_AND`/`BOOL_OR` and spells string aggregation with its own
/// `SEPARATOR` keyword rather than a second argument.
pub(crate) fn mysql(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        BoolAnd => t![L("(MIN("), A(0), L(") <> 0)")],
        BoolOr => t![L("(MAX("), A(0), L(") <> 0)")],
        GroupConcat => t![L("GROUP_CONCAT("), A(0), L(" SEPARATOR "), A(1), L(")")],
        StdDev => t![L("STDDEV_SAMP("), A(0), L(")")],
        Variance => t![L("VAR_SAMP("), A(0), L(")")],
        _ => return None,
    })
}

/// SQLite overrides.
///
/// SQLite has no statistical aggregates beyond the basics, so those return
/// `None` and are reported as unsupported rather than rendered into SQL that
/// would fail at execution time.
pub(crate) fn sqlite(op: Operator) -> Option<Template> {
    use Operator::*;
    Some(match op {
        BoolAnd => t![L("(MIN("), A(0), L(") <> 0)")],
        BoolOr => t![L("(MAX("), A(0), L(") <> 0)")],
        GroupConcat => t![L("GROUP_CONCAT("), A(0), L(", "), A(1), L(")")],
        _ => return None,
    })
}

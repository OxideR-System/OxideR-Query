//! Operator templates and precedence.
//!
//! A [`Template`] says how one operator's arguments are laid out in SQL text.
//! It is the Rust form of QueryDSL's template strings (`"{0} = {1}"`), but as a
//! `&'static` slice of elements instead of a string parsed by a regex at
//! runtime: the argument indices are checked when the table is written, there
//! is no parse step, and the table is still a flat, diffable list a dialect can
//! override row by row.
//!
//! Parenthesisation is not part of the template. It is decided once, centrally,
//! from the [`precedence`] ladder, exactly as QueryDSL decides it in
//! `SerializerBase.visitOperation` rather than in the template string.

use crate::ast::operator::Operator;

/// One piece of an operator's rendered form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elem {
    /// Verbatim SQL text.
    Lit(&'static str),
    /// The argument at this index, rendered in place.
    Arg(u8),
    /// The argument at this index rendered as a quoted identifier rather than
    /// as an expression, with each dot-separated part quoted separately so a
    /// schema-qualified name stays qualified. Used for sequence names, which
    /// engines take as names rather than as values.
    Ident(u8),
    /// The argument at this index rendered as a single-quoted SQL string
    /// literal, quotes included and any quote inside it doubled.
    ///
    /// PostgreSQL names a sequence with a string rather than an identifier
    /// (`NEXTVAL('seq')`), and that string is the one place the renderer builds
    /// a literal rather than binding a parameter, so it is escaped here rather
    /// than spliced by the template.
    TextLiteral(u8),
    /// Every remaining argument from this index on, comma separated. Used by
    /// variadic operators such as `COALESCE`.
    Rest(u8),
    /// Every argument, joined by this separator and each rendered at the
    /// operator's own precedence.
    ///
    /// Used by the associative connectives. `AND` and `OR` carry a flattened
    /// argument list rather than a left-leaning tree, so a filter built in a
    /// loop stays two levels deep instead of one level per condition, and the
    /// template has to render however many arguments turned up.
    Joined(&'static str),
}

/// How an operator lays out its arguments in SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Template(pub &'static [Elem]);

/// Build a template from a list of elements.
///
/// ```ignore
/// t![A(0), L(" = "), A(1)]   // renders as `lhs = rhs`
/// ```
macro_rules! t {
    ($($e:expr),* $(,)?) => { $crate::dialect::template::Template(&[$($e),*]) };
}
pub(crate) use t;

/// Precedence levels, ported from QueryDSL's `Templates.Precedence`.
///
/// Lower binds tighter. [`HIGHEST`] means "not applicable": the operator is
/// function-call shaped, so its own syntax already provides the grouping and
/// its arguments never need wrapping.
pub mod precedence {
    /// Function-call shaped; never needs parentheses.
    pub const HIGHEST: i16 = -1;
    /// Member access.
    pub const DOT: i16 = 5;
    /// Tight unary negation of a boolean.
    pub const NOT_HIGH: i16 = 10;
    /// Arithmetic negation.
    pub const NEGATE: i16 = 20;
    /// Multiplication, division, modulo.
    pub const ARITH_HIGH: i16 = 30;
    /// Addition, subtraction.
    pub const ARITH_LOW: i16 = 40;
    /// Ordering comparisons.
    pub const COMPARISON: i16 = 50;
    /// Equality comparisons.
    pub const EQUALITY: i16 = 60;
    /// `IS NULL` and friends, which every supported engine parses more loosely
    /// than the comparison operators.
    pub const IS: i16 = 65;
    /// CASE arms and comma-separated lists.
    pub const CASE: i16 = 70;
    /// Boolean NOT.
    pub const NOT: i16 = 80;
    /// Boolean AND.
    pub const AND: i16 = 90;
    /// Boolean XOR.
    pub const XOR: i16 = 100;
    /// Boolean OR.
    pub const OR: i16 = 110;
}

/// The ANSI precedence of an operator.
///
/// Dialects override this for the handful of operators whose grouping they are
/// fussier about; PostgreSQL and SQL Server both retune this ladder in
/// QueryDSL, and the same hook exists here on [`Dialect`](super::Dialect).
pub fn ansi_precedence(op: Operator) -> i16 {
    use precedence::*;
    use Operator::*;
    match op {
        Or => OR,
        And => AND,
        Not => NOT,
        // The ANSI template for XOR is `<>`, so its precedence has to be that
        // of the text actually emitted, not that of the XOR keyword. MySQL,
        // which has a real XOR operator, overrides this.
        Eq | Ne | IsDistinctFrom | IsNotDistinctFrom | EqIgnoreCase | Xor => EQUALITY,
        IsNull | IsNotNull => IS,
        Lt | Le | Gt | Ge | Between | NotBetween | In | NotIn | Like | LikeEscape | LikeIc
        | LikeEscapeIc | Matches | MatchesIc => COMPARISON,
        Add | Sub | Concat => ARITH_LOW,
        Mul | Div | Mod => ARITH_HIGH,
        Negate => NEGATE,
        // Everything else is a function call: `abs(x)`, `count(*)`, `coalesce(a, b)`.
        _ => HIGHEST,
    }
}

/// Whether a child expression needs wrapping in parentheses.
///
/// Ported from `SerializerBase.visitOperation`: a child is wrapped when the
/// parent binds tighter than the child, and never when the parent is
/// function-call shaped.
pub fn needs_parens(parent: i16, child: i16) -> bool {
    parent > precedence::HIGHEST && child > precedence::HIGHEST && parent < child
}

//! The operator catalog.
//!
//! Ported from QueryDSL's `Ops` (plus its nested `AggOps`, `QuantOps`,
//! `DateTimeOps`, `MathOps`, `StringOps`) and `SQLOps`, minus the operators that
//! only make sense for the JPA and in-memory-collection backends
//! (`INSTANCE_OF`, `COL_SIZE`, `MAP_IS_EMPTY`, and friends), plus the SQL
//! constructs QueryDSL never modelled: native `RETURNING`, `IS DISTINCT FROM`,
//! `SKIP LOCKED`, and the full window-frame vocabulary.
//!
//! This is deliberately one flat enum rather than several: rendering is a
//! single table lookup keyed by the operator, exactly as QueryDSL's
//! `Templates` keys an `IdentityHashMap<Operator, Template>`. Adding an
//! operator is one enum variant plus one row in the template table for the
//! dialects that need it; the renderer never changes. The per-family template
//! tables live in [`crate::dialect::templates`].

/// A SQL operator or function.
///
/// Which operators a given expression exposes is decided one layer up by trait
/// bounds on the Rust type (see [`crate::value`]); this enum is the untyped
/// vocabulary the renderer understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Operator {
    // -- Comparison ---------------------------------------------------------
    /// `a = b`
    Eq,
    /// `a <> b`
    Ne,
    /// `a < b`
    Lt,
    /// `a <= b`
    Le,
    /// `a > b`
    Gt,
    /// `a >= b`
    Ge,
    /// `a BETWEEN b AND c`
    Between,
    /// `a NOT BETWEEN b AND c`
    NotBetween,
    /// `a IS NULL`
    IsNull,
    /// `a IS NOT NULL`
    IsNotNull,
    /// `a IN (...)`
    In,
    /// `a NOT IN (...)`
    NotIn,
    /// `a IS DISTINCT FROM b` - null-safe inequality.
    IsDistinctFrom,
    /// `a IS NOT DISTINCT FROM b` - null-safe equality.
    IsNotDistinctFrom,

    // -- Boolean logic ------------------------------------------------------
    /// `a AND b`
    And,
    /// `a OR b`
    Or,
    /// `NOT a`
    Not,
    /// `a XOR b`
    Xor,

    // -- Quantifiers and existence -----------------------------------------
    /// `ANY (subquery)`
    Any,
    /// `ALL (subquery)`
    All,
    /// `EXISTS (subquery)`
    Exists,
    /// `NOT EXISTS (subquery)`
    NotExists,

    // -- Arithmetic ---------------------------------------------------------
    /// `a + b`
    Add,
    /// `a - b`
    Sub,
    /// `a * b`
    Mul,
    /// `a / b`
    Div,
    /// `a % b`
    Mod,
    /// `-a`
    Negate,

    // -- Math functions -----------------------------------------------------
    /// `ABS(a)`
    Abs,
    /// `CEIL(a)`
    Ceil,
    /// `FLOOR(a)`
    Floor,
    /// `ROUND(a)`
    Round,
    /// `ROUND(a, digits)`
    RoundTo,
    /// `SQRT(a)`
    Sqrt,
    /// `POWER(a, b)`
    Power,
    /// `EXP(a)`
    Exp,
    /// `LN(a)`
    Ln,
    /// `LOG(base, a)`
    Log,
    /// `SIGN(a)`
    Sign,
    /// `RANDOM()`
    Random,
    /// `RANDOM(seed)`
    RandomSeeded,
    /// `ACOS(a)`
    Acos,
    /// `ASIN(a)`
    Asin,
    /// `ATAN(a)`
    Atan,
    /// `COS(a)`
    Cos,
    /// `COSH(a)`
    Cosh,
    /// `COT(a)`
    Cot,
    /// `COTH(a)`
    Coth,
    /// `SIN(a)`
    Sin,
    /// `SINH(a)`
    Sinh,
    /// `TAN(a)`
    Tan,
    /// `TANH(a)`
    Tanh,
    /// `DEGREES(a)`
    Degrees,
    /// `RADIANS(a)`
    Radians,
    /// `LEAST(a, b)` - QueryDSL's `MathOps.MIN`.
    Least,
    /// `GREATEST(a, b)` - QueryDSL's `MathOps.MAX`.
    Greatest,

    // -- String functions ---------------------------------------------------
    /// `a || b`, or `CONCAT(a, b)` where that is the dialect's form.
    Concat,
    /// `LOWER(a)`
    Lower,
    /// `UPPER(a)`
    Upper,
    /// `TRIM(a)`
    Trim,
    /// `LTRIM(a)`
    LTrim,
    /// `RTRIM(a)`
    RTrim,
    /// `LENGTH(a)`
    Length,
    /// `SUBSTRING(a FROM start)`
    Substr,
    /// `SUBSTRING(a FROM start FOR len)`
    SubstrLen,
    /// `POSITION(needle IN haystack)`
    IndexOf,
    /// `POSITION(needle IN haystack)` starting at an offset.
    IndexOfFrom,
    /// `LEFT(a, n)`
    Left,
    /// `RIGHT(a, n)`
    Right,
    /// `LPAD(a, n)`
    LPad,
    /// `LPAD(a, n, fill)`
    LPadFill,
    /// `RPAD(a, n)`
    RPad,
    /// `RPAD(a, n, fill)`
    RPadFill,
    /// `REPLACE(a, from, to)`
    Replace,
    /// `a LIKE pattern`
    Like,
    /// `a LIKE pattern ESCAPE e`
    LikeEscape,
    /// Case-insensitive `LIKE` (`ILIKE` where native).
    LikeIc,
    /// Case-insensitive `LIKE ... ESCAPE`.
    LikeEscapeIc,
    /// `LOWER(a) = LOWER(b)`
    EqIgnoreCase,
    /// Regular-expression match.
    Matches,
    /// Case-insensitive regular-expression match.
    MatchesIc,
    /// `a = ''`
    StringIsEmpty,

    // -- Conditional --------------------------------------------------------
    /// `COALESCE(a, b, ...)`
    Coalesce,
    /// `NULLIF(a, b)`
    NullIf,

    // -- Date and time ------------------------------------------------------
    /// `CURRENT_DATE`
    CurrentDate,
    /// `CURRENT_TIME`
    CurrentTime,
    /// `CURRENT_TIMESTAMP`
    CurrentTimestamp,
    /// Cast a timestamp down to a date.
    DateOf,
    /// Extract the year.
    Year,
    /// Extract the month (1-12).
    Month,
    /// Extract the day of month.
    DayOfMonth,
    /// Extract the hour.
    Hour,
    /// Extract the minute.
    Minute,
    /// Extract the second.
    Second,
    /// Extract the millisecond.
    Millisecond,
    /// Extract the ISO week number.
    Week,
    /// Year and month combined as `YYYYMM`.
    YearMonth,
    /// Year and week combined as `YYYYWW`.
    YearWeek,
    /// Day of week (1 = Sunday).
    DayOfWeek,
    /// Day of year (1-366).
    DayOfYear,
    /// Add N years.
    AddYears,
    /// Add N months.
    AddMonths,
    /// Add N weeks.
    AddWeeks,
    /// Add N days.
    AddDays,
    /// Add N hours.
    AddHours,
    /// Add N minutes.
    AddMinutes,
    /// Add N seconds.
    AddSeconds,
    /// Whole years between two instants.
    DiffYears,
    /// Whole months between two instants.
    DiffMonths,
    /// Whole weeks between two instants.
    DiffWeeks,
    /// Whole days between two instants.
    DiffDays,
    /// Whole hours between two instants.
    DiffHours,
    /// Whole minutes between two instants.
    DiffMinutes,
    /// Whole seconds between two instants.
    DiffSeconds,
    /// Truncate to the start of the year.
    TruncYear,
    /// Truncate to the start of the month.
    TruncMonth,
    /// Truncate to the start of the week.
    TruncWeek,
    /// Truncate to the start of the day.
    TruncDay,
    /// Truncate to the start of the hour.
    TruncHour,
    /// Truncate to the start of the minute.
    TruncMinute,
    /// Truncate to the start of the second.
    TruncSecond,

    // -- Aggregates ---------------------------------------------------------
    /// `COUNT(*)`
    CountAll,
    /// `COUNT(a)`
    Count,
    /// `SUM(a)`
    Sum,
    /// `AVG(a)`
    Avg,
    /// `MIN(a)`
    Min,
    /// `MAX(a)`
    Max,
    /// `STDDEV(a)`
    StdDev,
    /// `STDDEV_POP(a)`
    StdDevPop,
    /// `STDDEV_SAMP(a)`
    StdDevSamp,
    /// `VARIANCE(a)`
    Variance,
    /// `VAR_POP(a)`
    VarPop,
    /// `VAR_SAMP(a)`
    VarSamp,
    /// `BOOL_AND(a)` - QueryDSL's `BOOLEAN_ALL`.
    BoolAnd,
    /// `BOOL_OR(a)` - QueryDSL's `BOOLEAN_ANY`.
    BoolOr,
    /// String aggregation with a separator.
    GroupConcat,
    /// `CORR(a, b)`
    Corr,
    /// `COVAR_POP(a, b)`
    CovarPop,
    /// `COVAR_SAMP(a, b)`
    CovarSamp,

    // -- Window and ranking functions ---------------------------------------
    /// `ROW_NUMBER()`
    RowNumber,
    /// `RANK()`
    Rank,
    /// `DENSE_RANK()`
    DenseRank,
    /// `PERCENT_RANK()`
    PercentRank,
    /// `CUME_DIST()`
    CumeDist,
    /// `NTILE(n)`
    Ntile,
    /// `LAG(a[, offset[, default]])`
    Lag,
    /// `LEAD(a[, offset[, default]])`
    Lead,
    /// `FIRST_VALUE(a)`
    FirstValue,
    /// `LAST_VALUE(a)`
    LastValue,
    /// `NTH_VALUE(a, n)`
    NthValue,
    /// `PERCENTILE_CONT(fraction) WITHIN GROUP (ORDER BY a)` - the
    /// interpolated value a fraction of the way through the sorted group.
    PercentileCont,
    /// `PERCENTILE_DISC(fraction) WITHIN GROUP (ORDER BY a)` - the first value
    /// at or past that fraction, taken from the group as it is.
    PercentileDisc,

    // -- Sequences ----------------------------------------------------------
    /// `NEXTVAL(seq)`
    NextVal,
    /// `CURRVAL(seq)`
    CurrVal,
}

/// Which family an operator belongs to.
///
/// Used to route template lookup to the right per-family table and to give
/// dialect authors a coarse switch when a whole family differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Comparison, null tests, `IN`, quantifiers, `EXISTS`.
    Comparison,
    /// Boolean connectives.
    Boolean,
    /// Arithmetic and math functions.
    Math,
    /// String functions and pattern matching.
    Text,
    /// Date and time functions.
    DateTime,
    /// Casts and conditional functions.
    Conditional,
    /// Aggregate functions.
    Aggregate,
    /// Window and ranking functions.
    Window,
    /// Sequence functions.
    Sequence,
}

impl Operator {
    /// The family this operator belongs to.
    pub fn family(self) -> Family {
        use Operator::*;
        match self {
            Eq | Ne | Lt | Le | Gt | Ge | Between | NotBetween | IsNull | IsNotNull | In
            | NotIn | IsDistinctFrom | IsNotDistinctFrom | Any | All | Exists | NotExists => {
                Family::Comparison
            }
            And | Or | Not | Xor => Family::Boolean,
            Add | Sub | Mul | Div | Mod | Negate | Abs | Ceil | Floor | Round | RoundTo | Sqrt
            | Power | Exp | Ln | Log | Sign | Random | RandomSeeded | Acos | Asin | Atan | Cos
            | Cosh | Cot | Coth | Sin | Sinh | Tan | Tanh | Degrees | Radians | Least
            | Greatest => Family::Math,
            Concat | Lower | Upper | Trim | LTrim | RTrim | Length | Substr | SubstrLen
            | IndexOf | IndexOfFrom | Left | Right | LPad | LPadFill | RPad | RPadFill
            | Replace | Like | LikeEscape | LikeIc | LikeEscapeIc | EqIgnoreCase | Matches
            | MatchesIc | StringIsEmpty => Family::Text,
            Coalesce | NullIf => Family::Conditional,
            CurrentDate | CurrentTime | CurrentTimestamp | DateOf | Year | Month | DayOfMonth
            | Hour | Minute | Second | Millisecond | Week | YearMonth | YearWeek | DayOfWeek
            | DayOfYear | AddYears | AddMonths | AddWeeks | AddDays | AddHours | AddMinutes
            | AddSeconds | DiffYears | DiffMonths | DiffWeeks | DiffDays | DiffHours
            | DiffMinutes | DiffSeconds | TruncYear | TruncMonth | TruncWeek | TruncDay
            | TruncHour | TruncMinute | TruncSecond => Family::DateTime,
            CountAll | Count | Sum | Avg | Min | Max | StdDev | StdDevPop | StdDevSamp
            | Variance | VarPop | VarSamp | BoolAnd | BoolOr | GroupConcat | Corr | CovarPop
            | CovarSamp | PercentileCont | PercentileDisc => Family::Aggregate,
            RowNumber | Rank | DenseRank | PercentRank | CumeDist | Ntile | Lag | Lead
            | FirstValue | LastValue | NthValue => Family::Window,
            NextVal | CurrVal => Family::Sequence,
        }
    }

    /// Whether this operator is an aggregate function, which decides whether it
    /// is legal in a HAVING clause and whether it can carry `DISTINCT` or a
    /// `FILTER` clause.
    pub fn is_aggregate(self) -> bool {
        self.family() == Family::Aggregate
    }

    /// Whether this is an ordered-set aggregate, whose sort is written as
    /// `WITHIN GROUP (ORDER BY ...)` after the call rather than inside it.
    ///
    /// The shape follows from the operator, so the renderer reads it from here
    /// rather than from a flag on the node, where the two could disagree.
    pub fn is_ordered_set(self) -> bool {
        matches!(self, Operator::PercentileCont | Operator::PercentileDisc)
    }
}

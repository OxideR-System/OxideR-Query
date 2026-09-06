//! Free-standing expression constructors.
//!
//! These are the entry points that do not hang off an existing expression:
//! literals, `NULL`, named parameters, the conditional functions, sequence
//! access, and the raw-SQL escape hatch. QueryDSL collects the same set in
//! `Expressions` and `SQLExpressions`.

use crate::ast::node::{ColumnRef, Node, RawPart, TableRef};
use crate::ast::operator::Operator;
use crate::source::{Concat, Nil};
use crate::typed::expr::{Expr, IntoExpr};
use crate::typed::ops_compare::Merge;
use crate::typed::selection::AnyExpr;
use crate::value::{Numeric, Orderable, ToSqlValue, Value};
use core::marker::PhantomData;

/// Bind a Rust value as a parameter.
///
/// Rarely needed, because every operand position already accepts a bare value.
/// It earns its place on the left of an operator (`val(1).add(Item::qty)`) and
/// where inference needs a nudge.
pub fn val<T: ToSqlValue>(value: T) -> Expr<Nil, T> {
    Expr::new(Node::Param(value.to_sql_value()))
}

/// The SQL literal `NULL`, typed as whatever `T` the context expects.
pub fn null<T>() -> Expr<Nil, T> {
    Expr::new(Node::Keyword("NULL"))
}

/// A named bind parameter, resolved before execution.
///
/// The building block for prepared statements reused with different values.
pub fn param<T>(name: &'static str) -> Expr<Nil, T> {
    Expr::new(Node::NamedParam(name))
}

/// Refer to a column by table qualifier and name, without a metamodel.
///
/// The escape hatch for columns no `Entity` describes: a derived table, a CTE,
/// or a view added after codegen ran. Because there is no entity behind it, the
/// expression's source set is empty and the compiler cannot check that the
/// qualifier is in scope - the responsibility moves to the caller.
pub fn col<T>(qualifier: &'static str, name: &'static str) -> Expr<Nil, T> {
    Expr::new(Node::Column(ColumnRef {
        table: TableRef::new(qualifier),
        name,
    }))
}

/// `*`, for a `SELECT *` projection.
pub fn star() -> Expr<Nil, ()> {
    Expr::new(Node::Star(None))
}

/// `<table>.*`, for selecting every column of one source in a join.
pub fn star_of(qualifier: &'static str) -> Expr<Nil, ()> {
    Expr::new(Node::Star(Some(TableRef::new(qualifier))))
}

/// Build a nullary function that takes no arguments and references nothing.
macro_rules! nullary {
    ($name:ident, $op:expr, $ret:ty, $doc:literal) => {
        #[doc = $doc]
        pub fn $name() -> Expr<Nil, $ret> {
            Expr::new(Node::op($op, []))
        }
    };
}

nullary!(random, Operator::Random, f64, "A random value in `[0, 1)`.");

/// The current date, as the dialect's `CURRENT_DATE`.
pub fn current_date<T>() -> Expr<Nil, T> {
    Expr::new(Node::op(Operator::CurrentDate, []))
}

/// The current wall-clock time, as the dialect's `CURRENT_TIME`.
pub fn current_time<T>() -> Expr<Nil, T> {
    Expr::new(Node::op(Operator::CurrentTime, []))
}

/// The current timestamp, as the dialect's `CURRENT_TIMESTAMP`.
pub fn current_timestamp<T>() -> Expr<Nil, T> {
    Expr::new(Node::op(Operator::CurrentTimestamp, []))
}

/// The next value of a database sequence.
///
/// The name is spliced into the SQL as an identifier, not bound as a parameter,
/// because no engine accepts a parameter there. Pass a literal name.
pub fn next_val<T>(sequence: &'static str) -> Expr<Nil, T> {
    Expr::new(Node::op(Operator::NextVal, [Node::Keyword(sequence)]))
}

/// The sequence's current value in this session.
pub fn curr_val<T>(sequence: &'static str) -> Expr<Nil, T> {
    Expr::new(Node::op(Operator::CurrVal, [Node::Keyword(sequence)]))
}

/// `NULLIF(a, b)` - NULL when the two are equal, else `a`.
pub fn nullif<A, B, T>(a: A, b: B) -> Expr<Merge<A::Sources, B::Sources>, T>
where
    A: IntoExpr<T>,
    B: IntoExpr<T>,
    A::Sources: Concat<B::Sources>,
{
    Expr::new(Node::binary(
        Operator::NullIf,
        a.into_expr_node(),
        b.into_expr_node(),
    ))
}

/// A variadic function call under construction: `COALESCE`, `LEAST`, or
/// `GREATEST`.
///
/// Arguments accumulate through [`or`](VarArgs::or) so each one carries its own
/// source set. A fixed-arity signature would need one overload per arity, and a
/// `Vec` argument would force every operand to have the same source set, which
/// rules out mixing a column with a literal.
pub struct VarArgs<S, T> {
    op: Operator,
    args: Vec<Node>,
    _marker: PhantomData<fn() -> (S, T)>,
}

impl<S, T> VarArgs<S, T> {
    /// Add another operand.
    pub fn or<X>(mut self, next: X) -> VarArgs<Merge<S, X::Sources>, T>
    where
        X: IntoExpr<T>,
        S: Concat<X::Sources>,
    {
        self.args.push(next.into_expr_node());
        VarArgs {
            op: self.op,
            args: self.args,
            _marker: PhantomData,
        }
    }

    /// Finish the call.
    pub fn end(self) -> Expr<S, T> {
        Expr::new(Node::Op(self.op, self.args))
    }
}

/// Start a `COALESCE(...)`: the first non-NULL operand wins.
///
/// The result keeps the type of the operands, so coalescing a nullable column
/// with a non-null default gives a non-null expression - which is the reason to
/// write it in the first place.
pub fn coalesce<X, T>(first: X) -> VarArgs<X::Sources, T>
where
    X: IntoExpr<T>,
{
    VarArgs {
        op: Operator::Coalesce,
        args: vec![first.into_expr_node()],
        _marker: PhantomData,
    }
}

/// Start a `LEAST(...)`: the smallest operand.
///
/// Renders as `MIN(...)` on SQLite, which spells the scalar form that way.
pub fn least<X, T>(first: X) -> VarArgs<X::Sources, T>
where
    X: IntoExpr<T>,
    T: Orderable,
{
    VarArgs {
        op: Operator::Least,
        args: vec![first.into_expr_node()],
        _marker: PhantomData,
    }
}

/// Start a `GREATEST(...)`: the largest operand.
pub fn greatest<X, T>(first: X) -> VarArgs<X::Sources, T>
where
    X: IntoExpr<T>,
    T: Orderable,
{
    VarArgs {
        op: Operator::Greatest,
        args: vec![first.into_expr_node()],
        _marker: PhantomData,
    }
}

/// `ROUND(value, digits)` written as a free function, for the case where the
/// operand is a literal.
pub fn round_to<X, T>(value: X, digits: i32) -> Expr<X::Sources, T>
where
    X: IntoExpr<T>,
    T: Numeric,
{
    Expr::new(Node::op(
        Operator::RoundTo,
        [
            value.into_expr_node(),
            Node::Param(Value::Int(digits as i64)),
        ],
    ))
}

/// A raw SQL fragment under construction.
///
/// The escape hatch for engine-specific syntax the operator catalog does not
/// cover. It is safe by construction: only [`sql`](Raw::sql) writes text into
/// the statement, and it takes `&'static str`, so no runtime value can reach
/// the SQL. Values go through [`bind`](Raw::bind) and expressions through
/// [`expr`](Raw::expr), both of which render as parameters.
///
/// ```ignore
/// let json_field: Expr<_, String> = Raw::new()
///     .expr(Event::payload)
///     .sql(" ->> ")
///     .bind("user_id")
///     .build();
/// ```
pub struct Raw<S> {
    parts: Vec<RawPart>,
    _marker: PhantomData<fn() -> S>,
}

impl Default for Raw<Nil> {
    fn default() -> Self {
        Self::new()
    }
}

impl Raw<Nil> {
    /// Start an empty fragment.
    pub fn new() -> Self {
        Raw {
            parts: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl<S> Raw<S> {
    /// Append verbatim SQL text.
    pub fn sql(mut self, text: &'static str) -> Self {
        self.parts.push(RawPart::Sql(text));
        self
    }

    /// Append an expression, rendered and parameterized normally.
    pub fn expr<X>(mut self, expr: X) -> Raw<Merge<S, X::Sources>>
    where
        X: AnyExpr,
        S: Concat<X::Sources>,
    {
        self.parts.push(RawPart::Expr(expr.into_any_node()));
        Raw {
            parts: self.parts,
            _marker: PhantomData,
        }
    }

    /// Append a bound value.
    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.parts.push(RawPart::Expr(Node::Param(value.into())));
        self
    }

    /// Finish, claiming the Rust type the fragment evaluates to.
    pub fn build<T>(self) -> Expr<S, T> {
        Expr::new(Node::Raw(self.parts))
    }
}

/// A whole raw fragment with no embedded expressions.
pub fn raw<T>(sql: &'static str) -> Expr<Nil, T> {
    Raw::new().sql(sql).build()
}

/// The current timestamp, typed as a `NaiveDateTime`.
///
/// The concrete-typed spelling of [`current_timestamp`], so the temporal
/// operators are reachable without naming the type.
#[cfg(feature = "chrono")]
pub fn now() -> Expr<Nil, chrono::NaiveDateTime> {
    current_timestamp()
}

/// The current date, typed as a `NaiveDate`.
#[cfg(feature = "chrono")]
pub fn today() -> Expr<Nil, chrono::NaiveDate> {
    current_date()
}

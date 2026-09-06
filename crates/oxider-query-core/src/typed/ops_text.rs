//! String functions and pattern matching, available on `String` expressions.
//!
//! The pattern-building helpers (`contains`, `starts_with`, `ends_with`) escape
//! the user's value before wrapping it in wildcards and emit an explicit
//! `ESCAPE` clause. QueryDSL only escapes constant operands and silently skips
//! it for dynamic ones, which means a search for `50%` there matches far more
//! than the user asked for. Escaping unconditionally is the whole point of
//! having a builder, so this port fixes that rather than reproducing it.

// `is_empty` takes `self` by value like every other builder here: an
// operand is moved into the node it becomes, and columns are `Copy`, so
// borrowing would only add a clone at the call site.
#![allow(clippy::wrong_self_convention)]

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::source::Concat;
use crate::typed::expr::{Expr, IntoExpr, Predicate};
use crate::typed::ops_compare::Merge;
use crate::value::Value;

/// The escape character used in generated `LIKE` patterns.
///
/// Deliberately not a backslash. A backslash means different things inside a
/// string literal on different engines - MySQL reads `'\\'` as one backslash
/// while PostgreSQL and SQLite read it as two - so no single `ESCAPE` clause
/// spelling works everywhere, and a query is built before its dialect is known.
/// `!` is an ordinary character in a string literal on every engine, so the
/// generated clause is identical on all of them.
pub const ESCAPE: char = '!';

/// The `ESCAPE` clause operand, written into the SQL rather than bound.
///
/// It is a constant of this crate and never user input, so inlining it cannot
/// inject anything - and it has to be inlined, because MySQL requires the
/// escape operand to be a literal rather than a parameter.
const ESCAPE_LITERAL: &str = "'!'";

/// Escape the `LIKE` metacharacters in a literal so it matches itself.
fn escape_like(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '%' || ch == '_' || ch == ESCAPE {
            out.push(ESCAPE);
        }
        out.push(ch);
    }
    out
}

/// String operations. Available when the expression's Rust type is `String`.
pub trait TextOps: IntoExpr<String> + Sized {
    /// `self LIKE pattern`, with the pattern used verbatim.
    ///
    /// Wildcards in `pattern` are meaningful. Use [`contains`](TextOps::contains)
    /// and friends when the value should be matched literally.
    fn like<R>(self, pattern: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::Like,
            self.into_expr_node(),
            pattern.into_expr_node(),
        ))
    }

    /// Case-insensitive `LIKE`, using `ILIKE` where the dialect has it.
    fn like_ignore_case<R>(self, pattern: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::LikeIc,
            self.into_expr_node(),
            pattern.into_expr_node(),
        ))
    }

    /// `self` contains `value` as a literal substring.
    fn contains(self, value: impl AsRef<str>) -> Predicate<Self::Sources> {
        self.like_literal(format!("%{}%", escape_like(value.as_ref())), false)
    }

    /// Case-insensitive [`contains`](TextOps::contains).
    fn contains_ignore_case(self, value: impl AsRef<str>) -> Predicate<Self::Sources> {
        self.like_literal(format!("%{}%", escape_like(value.as_ref())), true)
    }

    /// `self` starts with `value`, matched literally.
    fn starts_with(self, value: impl AsRef<str>) -> Predicate<Self::Sources> {
        self.like_literal(format!("{}%", escape_like(value.as_ref())), false)
    }

    /// Case-insensitive [`starts_with`](TextOps::starts_with).
    fn starts_with_ignore_case(self, value: impl AsRef<str>) -> Predicate<Self::Sources> {
        self.like_literal(format!("{}%", escape_like(value.as_ref())), true)
    }

    /// `self` ends with `value`, matched literally.
    fn ends_with(self, value: impl AsRef<str>) -> Predicate<Self::Sources> {
        self.like_literal(format!("%{}", escape_like(value.as_ref())), false)
    }

    /// Case-insensitive [`ends_with`](TextOps::ends_with).
    fn ends_with_ignore_case(self, value: impl AsRef<str>) -> Predicate<Self::Sources> {
        self.like_literal(format!("%{}", escape_like(value.as_ref())), true)
    }

    /// Build a `LIKE ... ESCAPE` predicate from an already-escaped pattern.
    #[doc(hidden)]
    fn like_literal(self, pattern: String, ignore_case: bool) -> Predicate<Self::Sources> {
        let op = if ignore_case {
            Operator::LikeEscapeIc
        } else {
            Operator::LikeEscape
        };
        Expr::new(Node::op(
            op,
            [
                self.into_expr_node(),
                Node::Param(Value::Text(pattern)),
                Node::Keyword(ESCAPE_LITERAL),
            ],
        ))
    }

    /// `LOWER(self) = LOWER(rhs)`, or the dialect's equivalent.
    fn eq_ignore_case<R>(self, rhs: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::EqIgnoreCase,
            self.into_expr_node(),
            rhs.into_expr_node(),
        ))
    }

    /// Regular-expression match. Not every engine has one; SQLite needs the
    /// host application to register a `regexp` function.
    fn matches<R>(self, pattern: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::Matches,
            self.into_expr_node(),
            pattern.into_expr_node(),
        ))
    }

    /// Case-insensitive regular-expression match.
    fn matches_ignore_case<R>(self, pattern: R) -> Predicate<Merge<Self::Sources, R::Sources>>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::MatchesIc,
            self.into_expr_node(),
            pattern.into_expr_node(),
        ))
    }

    /// `self = ''`
    fn is_empty(self) -> Predicate<Self::Sources> {
        Expr::new(Node::unary(Operator::StringIsEmpty, self.into_expr_node()))
    }

    /// `UPPER(self)`
    fn upper(self) -> Expr<Self::Sources, String> {
        self.text_fn(Operator::Upper)
    }

    /// `LOWER(self)`
    fn lower(self) -> Expr<Self::Sources, String> {
        self.text_fn(Operator::Lower)
    }

    /// `TRIM(self)`
    fn trim(self) -> Expr<Self::Sources, String> {
        self.text_fn(Operator::Trim)
    }

    /// `LTRIM(self)`
    fn trim_start(self) -> Expr<Self::Sources, String> {
        self.text_fn(Operator::LTrim)
    }

    /// `RTRIM(self)`
    fn trim_end(self) -> Expr<Self::Sources, String> {
        self.text_fn(Operator::RTrim)
    }

    /// `LENGTH(self)`, in characters.
    fn length(self) -> Expr<Self::Sources, i64> {
        Expr::new(Node::unary(Operator::Length, self.into_expr_node()))
    }

    /// Substring from `start` (1-based) to the end.
    fn substr(self, start: i64) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::Substr,
            [self.into_expr_node(), int_node(start)],
        ))
    }

    /// Substring of `len` characters from `start` (1-based).
    fn substr_len(self, start: i64, len: i64) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::SubstrLen,
            [self.into_expr_node(), int_node(start), int_node(len)],
        ))
    }

    /// The 1-based position of `needle` in `self`, or 0 when absent.
    fn index_of<R>(self, needle: R) -> Expr<Merge<Self::Sources, R::Sources>, i64>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::IndexOf,
            self.into_expr_node(),
            needle.into_expr_node(),
        ))
    }

    /// The leftmost `n` characters.
    fn left(self, n: i64) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::Left,
            [self.into_expr_node(), int_node(n)],
        ))
    }

    /// The rightmost `n` characters.
    fn right(self, n: i64) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::Right,
            [self.into_expr_node(), int_node(n)],
        ))
    }

    /// Pad on the left to `n` characters with `fill`.
    fn pad_start(self, n: i64, fill: impl AsRef<str>) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::LPadFill,
            [self.into_expr_node(), int_node(n), text_node(fill.as_ref())],
        ))
    }

    /// Pad on the right to `n` characters with `fill`.
    fn pad_end(self, n: i64, fill: impl AsRef<str>) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::RPadFill,
            [self.into_expr_node(), int_node(n), text_node(fill.as_ref())],
        ))
    }

    /// Replace every occurrence of `from` with `to`.
    fn replace(self, from: impl AsRef<str>, to: impl AsRef<str>) -> Expr<Self::Sources, String> {
        Expr::new(Node::op(
            Operator::Replace,
            [
                self.into_expr_node(),
                text_node(from.as_ref()),
                text_node(to.as_ref()),
            ],
        ))
    }

    /// Concatenate with another string expression.
    fn concat<R>(self, rhs: R) -> Expr<Merge<Self::Sources, R::Sources>, String>
    where
        R: IntoExpr<String>,
        Self::Sources: Concat<R::Sources>,
    {
        Expr::new(Node::binary(
            Operator::Concat,
            self.into_expr_node(),
            rhs.into_expr_node(),
        ))
    }

    /// Apply a single-argument string function.
    #[doc(hidden)]
    fn text_fn(self, op: Operator) -> Expr<Self::Sources, String> {
        Expr::new(Node::unary(op, self.into_expr_node()))
    }
}

fn int_node(n: i64) -> Node {
    Node::Param(Value::Int(n))
}

fn text_node(s: &str) -> Node {
    Node::Param(Value::Text(s.to_string()))
}

impl<E> TextOps for crate::typed::Column<E, String> {}
impl<S> TextOps for Expr<S, String> {}

#[cfg(test)]
mod tests {
    use super::escape_like;

    #[test]
    fn like_metacharacters_are_escaped_so_a_search_matches_itself() {
        assert_eq!(escape_like("50%"), "50!%");
        assert_eq!(escape_like("a_b"), "a!_b");
        assert_eq!(escape_like("wow!"), "wow!!");
        // A backslash is an ordinary character now that it is not the escape.
        assert_eq!(escape_like(r"c:\tmp"), r"c:\tmp");
        assert_eq!(escape_like("plain"), "plain");
    }
}

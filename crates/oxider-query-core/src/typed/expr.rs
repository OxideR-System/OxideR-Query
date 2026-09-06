//! The typed expression layer.
//!
//! [`Expr<S, T>`] is the one typed wrapper around the AST. `T` is the Rust type
//! the expression evaluates to, and gates which operators are available through
//! trait bounds; `S` is the type-level set of entities the expression
//! references, and is what makes "you never joined that table" a compile error.
//!
//! QueryDSL reaches the same place with a class hierarchy: `NumberExpression`
//! exposes arithmetic, `StringExpression` exposes `like`, and a `Path` is a
//! subclass of the expression type it produces. Rust does it with one type and
//! several bounded impls, which avoids the combinatorial explosion Java runs
//! into (`NumberExpression` there has to reimplement the comparison operators
//! because it cannot also extend `ComparableExpression`).

use crate::ast::node::Node;
use crate::ast::query::{NullsOrder, OrderAst, OrderDir};
use crate::dialect::CastKind;
use crate::source::{Cons, Nil};
use crate::value::ToSqlValue;
use core::marker::PhantomData;

/// A typed SQL expression: `T` is its Rust result type, `S` the set of entities
/// it references.
pub struct Expr<S, T> {
    node: Node,
    _marker: PhantomData<fn() -> (S, T)>,
}

impl<S, T> Expr<S, T> {
    /// Wrap an AST node as a typed expression.
    pub(crate) fn new(node: Node) -> Self {
        Expr {
            node,
            _marker: PhantomData,
        }
    }

    /// Consume into the underlying AST node.
    pub fn into_node(self) -> Node {
        self.node
    }

    /// Borrow the underlying AST node.
    pub fn node(&self) -> &Node {
        &self.node
    }

    /// Name this expression in a SELECT list: `expr AS alias`.
    pub fn alias(self, name: &'static str) -> Expr<S, T> {
        Expr::new(Node::Alias(Box::new(self.node), name))
    }

    /// Reinterpret the Rust type without changing the SQL.
    ///
    /// The escape hatch for domain types the library cannot infer, such as an
    /// enum stored as text. It asserts a type the database is trusted to
    /// produce, so it is the one place the type-safety is only as good as the
    /// caller's claim.
    pub fn coerce<U>(self) -> Expr<S, U> {
        Expr::new(self.node)
    }

    /// `CAST(self AS <type>)`, with the type named per dialect.
    pub fn cast<U>(self, kind: CastKind) -> Expr<S, U> {
        Expr::new(Node::Cast {
            expr: Box::new(self.node),
            kind,
        })
    }
}

impl<S, T> Clone for Expr<S, T> {
    fn clone(&self) -> Self {
        Expr::new(self.node.clone())
    }
}

impl<S, T> core::fmt::Debug for Expr<S, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Expr").field(&self.node).finish()
    }
}

/// A boolean expression: the type every clause taking a condition accepts.
///
/// This is a plain alias, so a predicate is an expression like any other and
/// can be selected, aliased, or nested inside `CASE`.
pub type Predicate<S> = Expr<S, bool>;

/// Anything usable where an expression of type `T` is expected.
///
/// Implemented for [`Expr`], for [`Column`](crate::typed::Column), and for
/// every value type that binds as a parameter. That last impl is what lets one
/// `eq` method accept both `User::age.eq(18)` and
/// `User::dept_id.eq(Department::id)`, where QueryDSL needs two overloads and
/// the previous version of this crate needed `eq` and `eq_column`.
///
/// The value impl is deliberately a blanket over [`ToSqlValue`] rather than
/// over `Into<T>`: the `Into` form collides with the `Column` impl under
/// Rust's coherence rules, so widening conversions are listed explicitly
/// instead.
pub trait IntoExpr<T> {
    /// The entities this operand references. `Nil` for a bound value.
    type Sources;

    /// Lower into an AST node.
    fn into_expr_node(self) -> Node;
}

impl<S, T> IntoExpr<T> for Expr<S, T> {
    type Sources = S;
    fn into_expr_node(self) -> Node {
        self.node
    }
}

impl<T: ToSqlValue> IntoExpr<T> for T {
    type Sources = Nil;
    fn into_expr_node(self) -> Node {
        Node::Param(self.to_sql_value())
    }
}

/// An optional value binds its content, or SQL NULL when there is none.
///
/// This is what lets an INSERT or UPDATE write an optional field directly:
/// `insert.set(User::email, maybe_email)`. In a comparison it means what it
/// says - `x = NULL` is never true, whatever `x` holds - so a nullability test
/// belongs in [`is_null`](crate::typed::CompareOps::is_null), not here.
impl<T: ToSqlValue> IntoExpr<T> for Option<T> {
    type Sources = Nil;
    fn into_expr_node(self) -> Node {
        Node::Param(self.to_sql_value())
    }
}

/// Widening conversions accepted wherever an expression of the wider type is
/// expected, so `name.eq("alice")` and `count.eq(1i32)` both work.
macro_rules! widening {
    ($($from:ty => $to:ty),* $(,)?) => {
        $(impl IntoExpr<$to> for $from {
            type Sources = Nil;
            fn into_expr_node(self) -> Node {
                Node::Param(<$to as From<$from>>::from(self).to_sql_value())
            }
        })*
    };
}

widening! {
    &str => String,
    i8 => i64,
    i16 => i64,
    i32 => i64,
    u8 => i64,
    u16 => i64,
    u32 => i64,
    f32 => f64,
    i8 => i32,
    i16 => i32,
    u8 => i32,
    u16 => i32,
}

/// `&String` is accepted where a `String` is expected, so a borrowed field can
/// be compared without cloning at the call site.
impl IntoExpr<String> for &String {
    type Sources = Nil;
    fn into_expr_node(self) -> Node {
        Node::Param(crate::value::Value::Text(self.clone()))
    }
}

/// A typed ORDER BY term, carrying the entities it references.
pub struct Order<S> {
    term: OrderAst,
    _marker: PhantomData<fn() -> S>,
}

impl<S> Order<S> {
    /// Build an order term.
    pub(crate) fn new(expr: Node, dir: OrderDir) -> Self {
        Order {
            term: OrderAst::new(expr, dir),
            _marker: PhantomData,
        }
    }

    /// Sort NULLs before non-NULL values.
    ///
    /// Emulated with a leading `CASE` sort key on engines without native
    /// support, so the ordering is the same everywhere.
    pub fn nulls_first(mut self) -> Self {
        self.term.nulls = NullsOrder::First;
        self
    }

    /// Sort NULLs after non-NULL values.
    pub fn nulls_last(mut self) -> Self {
        self.term.nulls = NullsOrder::Last;
        self
    }

    /// Consume into the erased AST term.
    pub(crate) fn into_term(self) -> OrderAst {
        self.term
    }
}

/// The source set of an expression referencing exactly one entity.
pub type Only<E> = Cons<E, Nil>;

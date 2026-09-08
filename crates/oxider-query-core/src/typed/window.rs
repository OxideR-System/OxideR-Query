//! Window functions: `OVER (PARTITION BY ... ORDER BY ... ROWS ...)`.
//!
//! A [`Window`] tracks the entities its partition and order expressions
//! reference, exactly as a predicate does, so a `PARTITION BY` over a table you
//! forgot to join is a compile error rather than a runtime one.

use crate::ast::node::Node;
use crate::ast::operator::Operator;
use crate::ast::window::{Frame, FrameBound, FrameExclusion, FrameUnit, InlineWindow, WindowSpec};
use crate::source::{Concat, Nil};
use crate::typed::aggregate::Aggregate;
use crate::typed::expr::{Expr, IntoExpr, Order};
use crate::typed::ops_compare::Merge;
use crate::typed::selection::AnyExpr;
use core::marker::PhantomData;

/// A window with no frame clause. The state every window starts in.
pub struct NoFrame;

/// A window carrying a frame clause.
///
/// `EXCLUDE` modifies a frame, so [`exclude`](Window::exclude) exists only
/// here. Asking to exclude rows from a window that has no frame used to be
/// silently dropped, leaving the window over the whole partition - a different
/// answer, arrived at without a word.
pub struct Framed;

/// A window specification under construction.
pub struct Window<S, Fr = NoFrame> {
    spec: InlineWindow,
    _marker: PhantomData<fn() -> (S, Fr)>,
}

impl Default for Window<Nil> {
    fn default() -> Self {
        Self::new()
    }
}

impl Window<Nil> {
    /// An empty window: the whole result set, in no particular order.
    pub fn new() -> Self {
        Window {
            spec: InlineWindow::default(),
            _marker: PhantomData,
        }
    }
}

impl<S, Fr> Window<S, Fr> {
    fn retype<S2, Fr2>(self) -> Window<S2, Fr2> {
        Window {
            spec: self.spec,
            _marker: PhantomData,
        }
    }

    /// Add a `PARTITION BY` expression. Any expression type may partition.
    pub fn partition_by<X>(mut self, expr: X) -> Window<Merge<S, X::Sources>, Fr>
    where
        X: AnyExpr,
        S: Concat<X::Sources>,
    {
        self.spec.partition_by.push(expr.into_any_node());
        self.retype()
    }

    /// Add an `ORDER BY` term.
    pub fn order_by<S2>(mut self, term: Order<S2>) -> Window<Merge<S, S2>, Fr>
    where
        S: Concat<S2>,
    {
        self.spec.order_by.push(term.into_term());
        self.retype()
    }

    /// Set a `ROWS` frame.
    pub fn rows(self, start: FrameBound, end: Option<FrameBound>) -> Window<S, Framed> {
        self.frame(FrameUnit::Rows, start, end)
    }

    /// Set a `RANGE` frame.
    pub fn range(self, start: FrameBound, end: Option<FrameBound>) -> Window<S, Framed> {
        self.frame(FrameUnit::Range, start, end)
    }

    /// Set a `GROUPS` frame.
    pub fn groups(self, start: FrameBound, end: Option<FrameBound>) -> Window<S, Framed> {
        self.frame(FrameUnit::Groups, start, end)
    }

    fn frame(
        mut self,
        unit: FrameUnit,
        start: FrameBound,
        end: Option<FrameBound>,
    ) -> Window<S, Framed> {
        self.spec.frame = Some(Frame {
            unit,
            start,
            end,
            exclusion: None,
        });
        self.retype()
    }

    /// Consume into the AST definition, for a query's `WINDOW` clause.
    pub fn into_definition(self) -> InlineWindow {
        self.spec
    }
}

impl<S> Window<S, Framed> {
    /// Add an `EXCLUDE` clause to the frame.
    pub fn exclude(mut self, exclusion: FrameExclusion) -> Self {
        let frame = self
            .spec
            .frame
            .as_mut()
            .expect("a Framed window always carries a frame");
        frame.exclusion = Some(exclusion);
        self
    }
}

impl<S, T> Aggregate<S, T> {
    /// Turn this aggregate into a window function over `window`.
    pub fn over<S2, Fr>(self, window: Window<S2, Fr>) -> Expr<Merge<S, S2>, T>
    where
        S: Concat<S2>,
    {
        Expr::new(Node::Window {
            func: Box::new(self.into_node()),
            spec: Box::new(WindowSpec::Inline(Box::new(window.into_definition()))),
        })
    }

    /// Turn this aggregate into a window function over a window declared in the
    /// query's `WINDOW` clause.
    pub fn over_named(self, name: &'static str) -> Expr<S, T> {
        Expr::new(Node::Window {
            func: Box::new(self.into_node()),
            spec: Box::new(WindowSpec::Named(name)),
        })
    }
}

/// Build a ranking function that takes no arguments.
macro_rules! ranking {
    ($name:ident, $op:expr, $ret:ty, $doc:literal) => {
        #[doc = $doc]
        pub fn $name() -> Aggregate<Nil, $ret> {
            Aggregate::new($op, Vec::new())
        }
    };
}

ranking!(
    row_number,
    Operator::RowNumber,
    i64,
    "The row's position within its window, starting at 1."
);
ranking!(
    rank,
    Operator::Rank,
    i64,
    "The row's rank, with gaps after ties."
);
ranking!(
    dense_rank,
    Operator::DenseRank,
    i64,
    "The row's rank, with no gaps after ties."
);
ranking!(
    percent_rank,
    Operator::PercentRank,
    f64,
    "The row's relative rank, between 0 and 1."
);
ranking!(
    cume_dist,
    Operator::CumeDist,
    f64,
    "The cumulative distribution of the row within its window."
);

/// Divide the window into `buckets` groups and return the row's bucket.
pub fn ntile(buckets: i64) -> Aggregate<Nil, i64> {
    Aggregate::new(
        Operator::Ntile,
        vec![Node::Param(crate::value::Value::Int(buckets))],
    )
}

/// The value of `expr` in the row `offset` positions before this one.
pub fn lag<X, T>(expr: X, offset: i64) -> Aggregate<X::Sources, T>
where
    X: IntoExpr<T>,
{
    Aggregate::new(
        Operator::Lag,
        vec![
            expr.into_expr_node(),
            Node::Param(crate::value::Value::Int(offset)),
        ],
    )
}

/// The value of `expr` in the row `offset` positions after this one.
pub fn lead<X, T>(expr: X, offset: i64) -> Aggregate<X::Sources, T>
where
    X: IntoExpr<T>,
{
    Aggregate::new(
        Operator::Lead,
        vec![
            expr.into_expr_node(),
            Node::Param(crate::value::Value::Int(offset)),
        ],
    )
}

/// The value of `expr` in the window's first row.
pub fn first_value<X, T>(expr: X) -> Aggregate<X::Sources, T>
where
    X: IntoExpr<T>,
{
    Aggregate::new(Operator::FirstValue, vec![expr.into_expr_node()])
}

/// The value of `expr` in the window's last row.
pub fn last_value<X, T>(expr: X) -> Aggregate<X::Sources, T>
where
    X: IntoExpr<T>,
{
    Aggregate::new(Operator::LastValue, vec![expr.into_expr_node()])
}

/// The value of `expr` in the window's `n`th row, counting from 1.
pub fn nth_value<X, T>(expr: X, n: i64) -> Aggregate<X::Sources, T>
where
    X: IntoExpr<T>,
{
    Aggregate::new(
        Operator::NthValue,
        vec![
            expr.into_expr_node(),
            Node::Param(crate::value::Value::Int(n)),
        ],
    )
}

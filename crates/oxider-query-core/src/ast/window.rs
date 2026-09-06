//! Window function specifications: `OVER (PARTITION BY ... ORDER BY ... ROWS ...)`.
//!
//! Mirrors QueryDSL's `WindowFunction`/`WindowOver`/`WindowRows` trio, but as
//! plain data rather than a builder chain: the typed layer builds the chain and
//! lowers it into this struct.

use crate::ast::node::Node;
use crate::ast::query::OrderAst;

/// The specification attached to a window function call.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowSpec {
    /// A window defined inline: `OVER (PARTITION BY ... ORDER BY ... <frame>)`.
    Inline(Box<InlineWindow>),
    /// A reference to a window declared in the query's `WINDOW` clause.
    Named(&'static str),
}

/// An inline window definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InlineWindow {
    /// `PARTITION BY` expressions, in order.
    pub partition_by: Vec<Node>,
    /// `ORDER BY` terms, in order.
    pub order_by: Vec<OrderAst>,
    /// The frame clause, if any.
    pub frame: Option<Frame>,
}

/// The unit a window frame counts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameUnit {
    /// `ROWS` - a physical count of rows.
    Rows,
    /// `RANGE` - a logical offset in the ordering value.
    Range,
    /// `GROUPS` - a count of peer groups.
    Groups,
}

impl FrameUnit {
    /// The SQL keyword for this unit.
    pub fn as_sql(self) -> &'static str {
        match self {
            FrameUnit::Rows => "ROWS",
            FrameUnit::Range => "RANGE",
            FrameUnit::Groups => "GROUPS",
        }
    }
}

/// One endpoint of a window frame.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameBound {
    /// `UNBOUNDED PRECEDING`.
    UnboundedPreceding,
    /// `<n> PRECEDING`.
    Preceding(u64),
    /// `CURRENT ROW`.
    CurrentRow,
    /// `<n> FOLLOWING`.
    Following(u64),
    /// `UNBOUNDED FOLLOWING`.
    UnboundedFollowing,
}

/// How rows outside the frame are treated at its edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameExclusion {
    /// `EXCLUDE CURRENT ROW`.
    CurrentRow,
    /// `EXCLUDE GROUP`.
    Group,
    /// `EXCLUDE TIES`.
    Ties,
    /// `EXCLUDE NO OTHERS`.
    NoOthers,
}

impl FrameExclusion {
    /// The SQL text for this exclusion.
    pub fn as_sql(self) -> &'static str {
        match self {
            FrameExclusion::CurrentRow => "EXCLUDE CURRENT ROW",
            FrameExclusion::Group => "EXCLUDE GROUP",
            FrameExclusion::Ties => "EXCLUDE TIES",
            FrameExclusion::NoOthers => "EXCLUDE NO OTHERS",
        }
    }
}

/// A window frame: `ROWS BETWEEN <start> AND <end>`, or the one-sided
/// `ROWS <start>` shorthand.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The counting unit.
    pub unit: FrameUnit,
    /// The frame start.
    pub start: FrameBound,
    /// The frame end; `None` renders the one-sided form.
    pub end: Option<FrameBound>,
    /// Optional `EXCLUDE` clause.
    pub exclusion: Option<FrameExclusion>,
}

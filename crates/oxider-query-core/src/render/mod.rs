//! Turns a dialect-agnostic AST into SQL text plus ordered bound parameters.
//!
//! The renderer walks the AST once, writing into a single `String` and pushing
//! every bound value onto one `Vec` so placeholder numbering stays globally
//! consistent, including across nested subqueries.
//!
//! It borrows the dialect as a trait object rather than a generic parameter.
//! Rendering is not hot enough to need monomorphisation, and one copy of the
//! walker keeps compile times down - a deliberate reaction to the reputation
//! Rust query builders have for slow builds.

mod dml;
mod expr;
mod select;

use crate::ast::operator::Operator;
use crate::dialect::Dialect;
use crate::value::Value;

pub use dml::{render_delete, render_insert, render_update};
pub use select::render_select_into;

/// Rendered SQL alongside its ordered bound parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    /// The SQL string, with dialect-specific placeholders.
    pub sql: String,
    /// Bound parameters, in placeholder order.
    pub params: Vec<Value>,
}

/// Why a query could not be rendered for a dialect.
///
/// Rendering is fallible because a query is built without knowing its target:
/// `FULL JOIN` is valid SQL and valid to build, but MySQL cannot run it. Saying
/// so here is better than emitting SQL that fails at the database with a
/// message that points at the wrong thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The dialect has no way to express this operator.
    UnsupportedOperator {
        /// The dialect that refused it.
        dialect: &'static str,
        /// The operator with no template.
        operator: Operator,
    },
    /// The dialect does not support this construct.
    UnsupportedFeature {
        /// The dialect that refused it.
        dialect: &'static str,
        /// A short description of the construct, for the error message.
        feature: &'static str,
    },
    /// A template referenced an argument the operator node does not have. A bug
    /// in a template table rather than in user code.
    MissingArgument {
        /// The operator whose template is wrong.
        operator: Operator,
        /// The index the template asked for.
        index: u8,
    },
    /// The query is structurally invalid, for example a derived table with no
    /// alias.
    Invalid(&'static str),
}

impl core::fmt::Display for RenderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RenderError::UnsupportedOperator { dialect, operator } => {
                write!(f, "{dialect} cannot express the operator {operator:?}")
            }
            RenderError::UnsupportedFeature { dialect, feature } => {
                write!(f, "{dialect} does not support {feature}")
            }
            RenderError::MissingArgument { operator, index } => write!(
                f,
                "the template for {operator:?} asked for argument {index}, which is missing"
            ),
            RenderError::Invalid(what) => write!(f, "invalid query: {what}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// The result of a rendering step.
pub type RenderResult<T = ()> = Result<T, RenderError>;

/// Accumulates SQL text and bound parameters during a render.
pub(crate) struct Renderer<'d> {
    dialect: &'d dyn Dialect,
    sql: String,
    params: Vec<Value>,
}

impl<'d> Renderer<'d> {
    /// Start rendering for a dialect.
    pub(crate) fn new(dialect: &'d dyn Dialect) -> Self {
        Renderer {
            dialect,
            sql: String::with_capacity(128),
            params: Vec::new(),
        }
    }

    /// Finish, yielding the SQL and its parameters.
    pub(crate) fn finish(self) -> Rendered {
        Rendered {
            sql: self.sql,
            params: self.params,
        }
    }

    /// The dialect being rendered for.
    pub(crate) fn dialect(&self) -> &'d dyn Dialect {
        self.dialect
    }

    /// Append verbatim SQL text.
    pub(crate) fn push(&mut self, text: &str) {
        self.sql.push_str(text);
    }

    /// Append a quoted identifier.
    pub(crate) fn ident(&mut self, ident: &str) {
        let quoted = self.dialect.quote_ident(ident);
        self.sql.push_str(&quoted);
    }

    /// Bind a value and append its placeholder.
    pub(crate) fn bind(&mut self, value: &Value) {
        self.params.push(value.clone());
        let placeholder = self.dialect.placeholder(self.params.len());
        self.sql.push_str(&placeholder);
    }

    /// Append a comma-separated list, rendering each item with `each`.
    pub(crate) fn comma_separated<T>(
        &mut self,
        items: &[T],
        mut each: impl FnMut(&mut Self, &T) -> RenderResult,
    ) -> RenderResult {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            each(self, item)?;
        }
        Ok(())
    }

    /// Report that the dialect cannot express a construct.
    pub(crate) fn unsupported<T>(&self, feature: &'static str) -> RenderResult<T> {
        Err(RenderError::UnsupportedFeature {
            dialect: self.dialect.name(),
            feature,
        })
    }
}

/// Anything that can be finalized into a [`Rendered`] statement for a dialect.
///
/// The execution layer takes `impl Renderable` rather than a pre-rendered
/// statement, so a call site passes a query directly and the connected database
/// picks the dialect. Queries stay dialect-agnostic, no `.render(&Dialect)` is
/// spelled at each call site, and switching databases does not touch query
/// code.
pub trait Renderable {
    /// Render into SQL text plus ordered bound parameters for `dialect`.
    fn render_with(self, dialect: &dyn Dialect) -> RenderResult<Rendered>;
}

impl Renderable for Rendered {
    /// Already rendered: the dialect is ignored and the statement returned as is.
    fn render_with(self, _dialect: &dyn Dialect) -> RenderResult<Rendered> {
        Ok(self)
    }
}

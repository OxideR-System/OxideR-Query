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
    /// A named parameter was left without a value.
    ///
    /// [`param`](crate::typed::param) puts a placeholder in the query and
    /// `bind` fills it in; rendering with one still empty would either bind
    /// nothing or bind the wrong thing, so it is refused instead.
    UnboundParameter {
        /// The parameter with no value.
        name: &'static str,
    },
    /// The expression nests deeper than the renderer will walk.
    ///
    /// The walk is recursive, so an unbounded depth means a stack overflow,
    /// which aborts the process instead of raising something a caller could
    /// handle. Reaching this limit means a query was built in a loop - a
    /// thousand predicates chained with `AND` rather than one `IN` - so the
    /// error names the depth to make the shape obvious.
    TooDeep {
        /// The depth the renderer refuses to go past.
        limit: u16,
    },
}

/// The deepest expression or subquery nesting [`Renderer`] will walk.
///
/// Measured rather than guessed: on the tightest budget a caller realistically
/// has (a Windows main thread, 1 MB of stack, unoptimised build) a chain of 512
/// predicates renders and 1024 overflows, so this leaves a factor of two.
/// No hand-written query comes close; a query that does was built in a loop.
pub const MAX_DEPTH: u16 = 256;

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
            RenderError::UnboundParameter { name } => {
                write!(f, "the named parameter `{name}` was never bound")
            }
            RenderError::TooDeep { limit } => write!(
                f,
                "the query nests more than {limit} levels deep; build it with one \
                 IN or a joined subquery rather than a chain of predicates"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// The result of a rendering step.
pub type RenderResult<T = ()> = Result<T, RenderError>;

/// Values for the named parameters a statement carries.
///
/// A query written with [`param`](crate::typed::param) is a template: the same
/// statement renders again with different values, without rebuilding it and
/// without the values ever reaching the SQL text. The lookup is a short linear
/// scan, which for the handful of parameters a statement has beats a map.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Bindings(Vec<(&'static str, Value)>);

impl Bindings {
    /// No bindings.
    pub fn new() -> Self {
        Bindings(Vec::new())
    }

    /// Give `name` a value, replacing any value it already had.
    pub fn set(mut self, name: &'static str, value: impl Into<Value>) -> Self {
        let value = value.into();
        match self.0.iter_mut().find(|(known, _)| *known == name) {
            Some(slot) => slot.1 = value,
            None => self.0.push((name, value)),
        }
        self
    }

    /// Whether anything has been bound.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The value bound to `name`, if any.
    fn get(&self, name: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(known, _)| *known == name)
            .map(|(_, value)| value)
    }
}

/// Accumulates SQL text and bound parameters during a render.
pub(crate) struct Renderer<'d> {
    dialect: &'d dyn Dialect,
    sql: String,
    params: Vec<Value>,
    /// Values for the statement's named parameters, from the outermost
    /// statement, so a `param` inside a subquery resolves too.
    bindings: &'d Bindings,
    /// How many levels of expression or subquery the walk is currently inside.
    depth: u16,
}

impl<'d> Renderer<'d> {
    /// Start rendering for a dialect, resolving named parameters from
    /// `bindings`.
    pub(crate) fn new(dialect: &'d dyn Dialect, bindings: &'d Bindings) -> Self {
        Renderer {
            dialect,
            sql: String::with_capacity(128),
            params: Vec::new(),
            bindings,
            depth: 0,
        }
    }

    /// Bind the value of a named parameter and append its placeholder.
    pub(crate) fn named(&mut self, name: &'static str) -> RenderResult {
        match self.bindings.get(name) {
            Some(value) => {
                let value = value.clone();
                self.bind(&value);
                Ok(())
            }
            None => Err(RenderError::UnboundParameter { name }),
        }
    }

    /// Run `body` one level deeper, refusing rather than overflowing the stack.
    ///
    /// Every recursive step in the walk goes through here, so the guard cannot
    /// be forgotten by a new node kind: the recursion and the counter are the
    /// same call.
    pub(crate) fn nested<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> RenderResult<T>,
    ) -> RenderResult<T> {
        if self.depth >= MAX_DEPTH {
            return Err(RenderError::TooDeep { limit: MAX_DEPTH });
        }
        self.depth += 1;
        let out = body(self);
        self.depth -= 1;
        out
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

    /// Append a possibly-qualified name, quoting each dot-separated part.
    ///
    /// `app.orders_seq` becomes `"app"."orders_seq"` rather than one identifier
    /// that happens to contain a dot, which is what an engine means by a
    /// qualified sequence or table name.
    pub(crate) fn qualified_ident(&mut self, name: &str) {
        for (i, part) in name.split('.').enumerate() {
            if i > 0 {
                self.push(".");
            }
            self.ident(part);
        }
    }

    /// Append a single-quoted SQL string literal, doubling any quote inside it.
    ///
    /// Only reached for names an engine spells as a string rather than as an
    /// identifier. Values never come through here; they are bound.
    pub(crate) fn text_literal(&mut self, text: &str) {
        self.sql.push('\'');
        for ch in text.chars() {
            if ch == '\'' {
                self.sql.push('\'');
            }
            self.sql.push(ch);
        }
        self.sql.push('\'');
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

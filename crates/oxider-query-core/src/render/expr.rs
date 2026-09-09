//! Expression rendering: templates, precedence, and the constructs that need
//! more than a template.

use crate::ast::node::{ColumnRef, Node, RawPart, TableRef};
use crate::ast::operator::Operator;
use crate::ast::query::OrderAst;
use crate::ast::window::{Frame, FrameBound, InlineWindow, WindowSpec};
use crate::dialect::template::{needs_parens, precedence};
use crate::dialect::Elem;
use crate::render::{RenderError, RenderResult, Renderer};

impl Renderer<'_> {
    /// Render an expression at the top level, where nothing can require
    /// parentheses around it.
    pub(crate) fn expr(&mut self, node: &Node) -> RenderResult {
        self.expr_within(node, precedence::HIGHEST)
    }

    /// Render an expression as the child of an operator of precedence
    /// `parent`, wrapping it in parentheses when the parent binds tighter.
    pub(crate) fn expr_within(&mut self, node: &Node, parent: i16) -> RenderResult {
        let child = match node {
            Node::Op(op, _) => self.dialect().precedence(*op),
            _ => precedence::HIGHEST,
        };
        let wrap = needs_parens(parent, child);
        if wrap {
            self.push("(");
        }
        // Every expression recursion funnels through here, so this is the one
        // place the depth guard has to sit.
        self.nested(|r| r.node(node))?;
        if wrap {
            self.push(")");
        }
        Ok(())
    }

    /// Render one node, without considering the surrounding precedence.
    fn node(&mut self, node: &Node) -> RenderResult {
        match node {
            Node::Column(col) => self.column_ref(col),
            Node::Param(value) => self.bind(value),
            Node::NamedParam(name) => return self.named(name),
            Node::Keyword(text) => self.push(text),
            Node::Star(table) => {
                if let Some(table) = table {
                    self.qualifier(table);
                    self.push(".");
                }
                self.push("*");
            }
            Node::Op(op, args) => return self.operation(*op, args),
            Node::Aggregate {
                func,
                distinct,
                args,
                order_by,
                filter,
            } => return self.aggregate(*func, *distinct, args, order_by, filter.as_deref()),
            Node::Cast { expr, kind } => {
                let type_name = self.dialect().cast_type(*kind);
                self.push("CAST(");
                self.expr(expr)?;
                self.push(" AS ");
                self.push(type_name);
                self.push(")");
            }
            Node::Case {
                operand,
                arms,
                otherwise,
            } => return self.case(operand.as_deref(), arms, otherwise.as_deref()),
            Node::Window { func, spec } => {
                self.expr(func)?;
                self.push(" OVER ");
                return self.window_spec(spec);
            }
            Node::Subquery(query) => {
                self.push("(");
                self.select(query)?;
                self.push(")");
            }
            Node::Row(items) => {
                self.push("(");
                self.comma_separated(items, |r, item| r.expr(item))?;
                self.push(")");
            }
            Node::Excluded(column) => {
                let caps = self.dialect().caps();
                if caps.on_conflict {
                    self.push("excluded.");
                    self.ident(column);
                } else if caps.on_duplicate_key {
                    self.push("VALUES(");
                    self.ident(column);
                    self.push(")");
                } else {
                    return self.unsupported("the excluded row of an upsert");
                }
            }
            Node::Alias(inner, alias) => {
                self.expr(inner)?;
                self.push(" AS ");
                self.ident(alias);
            }
            Node::Raw(parts) => {
                for part in parts {
                    match part {
                        RawPart::Sql(text) => self.push(text),
                        RawPart::Expr(node) => self.expr(node)?,
                    }
                }
            }
        }
        Ok(())
    }

    /// Render a column reference, qualified by its table's alias or name.
    fn column_ref(&mut self, col: &ColumnRef) {
        self.qualifier(&col.table);
        self.push(".");
        self.ident(col.name);
    }

    /// Render the identifier a table's columns are qualified with.
    pub(crate) fn qualifier(&mut self, table: &TableRef) {
        self.ident(table.qualifier());
    }

    /// Render an operator by expanding its dialect template.
    fn operation(&mut self, op: Operator, args: &[Node]) -> RenderResult {
        let Some(template) = self.dialect().template(op) else {
            return Err(RenderError::UnsupportedOperator {
                dialect: self.dialect().name(),
                operator: op,
            });
        };
        let own = self.dialect().precedence(op);
        for element in template.0 {
            match *element {
                Elem::Lit(text) => self.push(text),
                Elem::Arg(index) => {
                    let arg = arg_at(op, args, index)?;
                    self.expr_within(arg, own)?;
                }
                Elem::Ident(index) => {
                    let arg = arg_at(op, args, index)?;
                    match arg {
                        // A name the engine takes as a name rather than as a
                        // value, so it is quoted as an identifier - per
                        // dot-separated part, so `app.orders_seq` stays
                        // schema-qualified instead of becoming one odd name.
                        Node::Keyword(name) => self.qualified_ident(name),
                        other => self.expr(other)?,
                    }
                }
                Elem::TextLiteral(index) => {
                    let arg = arg_at(op, args, index)?;
                    match arg {
                        Node::Keyword(name) => self.text_literal(name),
                        other => self.expr(other)?,
                    }
                }
                Elem::Rest(from) => {
                    let rest = args.get(from as usize..).unwrap_or(&[]);
                    self.comma_separated(rest, |r, arg| r.expr(arg))?;
                }
                Elem::Joined(separator) => {
                    if args.is_empty() {
                        return Err(RenderError::MissingArgument {
                            operator: op,
                            index: 0,
                        });
                    }
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.push(separator);
                        }
                        self.expr_within(arg, own)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Render an aggregate call, including `DISTINCT`, an ordered argument
    /// list, and `FILTER (WHERE ...)` or its emulation.
    fn aggregate(
        &mut self,
        func: Operator,
        distinct: bool,
        args: &[Node],
        order_by: &[OrderAst],
        filter: Option<&Node>,
    ) -> RenderResult {
        // Engines without FILTER get the equivalent CASE folded into the
        // aggregate's argument, which is what the clause means anyway.
        if let Some(predicate) = filter {
            if !self.dialect().caps().aggregate_filter {
                let (func, args) = fold_filter_into_argument(func, args, predicate);
                return self.aggregate(func, distinct, &args, order_by, None);
            }
        }

        // An ordered-set aggregate sorts the group rather than the argument, so
        // its ORDER BY lands after the call instead of inside it. Nothing can
        // stand in for the clause where it is missing - a median is a property
        // of the whole sorted group, not of any row - so it is refused there.
        let ordered_set = func.is_ordered_set();
        if ordered_set {
            if !self.dialect().caps().ordered_set_aggregates {
                return self.unsupported("an ordered-set aggregate (WITHIN GROUP)");
            }
            if order_by.is_empty() {
                return Err(RenderError::Invalid(
                    "an ordered-set aggregate needs a WITHIN GROUP sort",
                ));
            }
        }

        let Some(template) = self.dialect().template(func) else {
            return Err(RenderError::UnsupportedOperator {
                dialect: self.dialect().name(),
                operator: func,
            });
        };

        for element in template.0 {
            match *element {
                Elem::Lit(text) => self.push(text),
                Elem::Arg(index) => {
                    if index == 0 && distinct {
                        self.push("DISTINCT ");
                    }
                    let arg = arg_at(func, args, index)?;
                    self.expr(arg)?;
                    // An ordered aggregate puts its ORDER BY inside the call,
                    // right after the aggregated expression.
                    if index == 0 && !ordered_set && !order_by.is_empty() {
                        self.push(" ORDER BY ");
                        self.order_terms(order_by)?;
                    }
                }
                Elem::Ident(index) | Elem::TextLiteral(index) => {
                    let arg = arg_at(func, args, index)?;
                    self.expr(arg)?;
                }
                Elem::Rest(from) => {
                    if distinct {
                        self.push("DISTINCT ");
                    }
                    let rest = args.get(from as usize..).unwrap_or(&[]);
                    self.comma_separated(rest, |r, arg| r.expr(arg))?;
                }
                // No aggregate is an associative connective, so this cannot be
                // reached from the template tables; render it consistently
                // anyway rather than silently emitting nothing.
                Elem::Joined(separator) => {
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.push(separator);
                        }
                        self.expr(arg)?;
                    }
                }
            }
        }

        if ordered_set {
            self.push(" WITHIN GROUP (ORDER BY ");
            self.order_terms(order_by)?;
            self.push(")");
        }

        if let Some(predicate) = filter {
            self.push(" FILTER (WHERE ");
            self.expr(predicate)?;
            self.push(")");
        }
        Ok(())
    }

    /// Render a CASE expression, simple or searched.
    fn case(
        &mut self,
        operand: Option<&Node>,
        arms: &[crate::ast::node::WhenArm],
        otherwise: Option<&Node>,
    ) -> RenderResult {
        self.push("CASE");
        if let Some(operand) = operand {
            self.push(" ");
            self.expr(operand)?;
        }
        for arm in arms {
            self.push(" WHEN ");
            self.expr(&arm.when)?;
            self.push(" THEN ");
            self.expr(&arm.then)?;
        }
        if let Some(otherwise) = otherwise {
            self.push(" ELSE ");
            self.expr(otherwise)?;
        }
        self.push(" END");
        Ok(())
    }

    /// Render an `OVER (...)` specification, or a named window reference.
    fn window_spec(&mut self, spec: &WindowSpec) -> RenderResult {
        if !self.dialect().caps().window_functions {
            return self.unsupported("window functions");
        }
        match spec {
            WindowSpec::Named(name) => {
                if !self.dialect().caps().named_windows {
                    return self.unsupported("named windows");
                }
                self.ident(name);
                Ok(())
            }
            WindowSpec::Inline(window) => {
                self.push("(");
                self.inline_window(window)?;
                self.push(")");
                Ok(())
            }
        }
    }

    /// Render the body of an inline window definition.
    pub(crate) fn inline_window(&mut self, window: &InlineWindow) -> RenderResult {
        let mut leading = false;
        if !window.partition_by.is_empty() {
            self.push("PARTITION BY ");
            self.comma_separated(&window.partition_by, |r, node| r.expr(node))?;
            leading = true;
        }
        if !window.order_by.is_empty() {
            if leading {
                self.push(" ");
            }
            self.push("ORDER BY ");
            self.order_terms(&window.order_by)?;
            leading = true;
        }
        if let Some(frame) = &window.frame {
            if leading {
                self.push(" ");
            }
            self.frame(frame)?;
        }
        Ok(())
    }

    /// Render a window frame clause.
    fn frame(&mut self, frame: &Frame) -> RenderResult {
        self.push(frame.unit.as_sql());
        self.push(" ");
        match &frame.end {
            Some(end) => {
                self.push("BETWEEN ");
                self.frame_bound(&frame.start);
                self.push(" AND ");
                self.frame_bound(end);
            }
            None => self.frame_bound(&frame.start),
        }
        if let Some(exclusion) = frame.exclusion {
            self.push(" ");
            self.push(exclusion.as_sql());
        }
        Ok(())
    }

    /// Render one endpoint of a window frame.
    fn frame_bound(&mut self, bound: &FrameBound) {
        match bound {
            FrameBound::UnboundedPreceding => self.push("UNBOUNDED PRECEDING"),
            FrameBound::Preceding(n) => {
                self.push(&n.to_string());
                self.push(" PRECEDING");
            }
            FrameBound::CurrentRow => self.push("CURRENT ROW"),
            FrameBound::Following(n) => {
                self.push(&n.to_string());
                self.push(" FOLLOWING");
            }
            FrameBound::UnboundedFollowing => self.push("UNBOUNDED FOLLOWING"),
        }
    }
}

/// Fetch a template's argument, reporting a template bug when it is missing.
fn arg_at(op: Operator, args: &[Node], index: u8) -> RenderResult<&Node> {
    args.get(index as usize)
        .ok_or(RenderError::MissingArgument {
            operator: op,
            index,
        })
}

/// Rewrite `agg(x) FILTER (WHERE p)` into `agg(CASE WHEN p THEN x END)`, the
/// portable equivalent for engines without the clause.
///
/// `COUNT(*)` has no argument to wrap, so it becomes `COUNT(CASE WHEN p THEN 1
/// END)`, which counts exactly the matching rows.
fn fold_filter_into_argument(
    func: Operator,
    args: &[Node],
    predicate: &Node,
) -> (Operator, Vec<Node>) {
    let guarded = |value: Node| Node::Case {
        operand: None,
        arms: vec![crate::ast::node::WhenArm {
            when: predicate.clone(),
            then: value,
        }],
        otherwise: None,
    };

    if func == Operator::CountAll {
        return (Operator::Count, vec![guarded(Node::Keyword("1"))]);
    }

    let mut rewritten = args.to_vec();
    if let Some(first) = rewritten.first_mut() {
        *first = guarded(first.clone());
    }
    (func, rewritten)
}

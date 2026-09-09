//! SQL dialect abstraction.
//!
//! A dialect answers three kinds of question:
//!
//! 1. Lexical: how are identifiers quoted, how is a bound parameter spelled.
//! 2. Vocabulary: which SQL text does each operator expand to. Answered by a
//!    flat, overridable template table (see [`templates`]).
//! 3. Structure: does this engine support `NULLS LAST`, `FILTER (WHERE ...)`,
//!    `RETURNING`, `SKIP LOCKED`. Answered by [`Caps`], which the renderer
//!    consults to pick a native clause or an emulation.
//!
//! The split matters. QueryDSL learned it the hard way: its `SQLTemplates` is a
//! data table for the easy 90% of operators, but pagination, MERGE, and null
//! ordering are method overrides, because they restructure the query rather
//! than substitute text. Modelling both from the start keeps the table honest.

mod mysql;
mod postgres;
mod sqlite;
pub mod template;
pub mod templates;

pub use mysql::MySql;
pub use postgres::Postgres;
pub use sqlite::Sqlite;
pub use template::{needs_parens, precedence, Elem, Template};

use crate::ast::operator::Operator;

/// The target type of a cast, named per dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastKind {
    /// Signed integer.
    Integer,
    /// Floating point.
    Float,
    /// Arbitrary-precision decimal.
    Decimal,
    /// Text.
    Text,
    /// Boolean.
    Bool,
    /// Calendar date.
    Date,
    /// Wall-clock time.
    Time,
    /// Timestamp.
    DateTime,
}

/// What a dialect structurally supports.
///
/// The renderer reads these to choose between a native clause and an
/// emulation, or to refuse to render something that would only fail later at
/// the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// Native `NULLS FIRST` / `NULLS LAST` in ORDER BY.
    pub nulls_ordering: bool,
    /// `SELECT DISTINCT ON (...)`.
    pub distinct_on: bool,
    /// `FILTER (WHERE ...)` on aggregates.
    pub aggregate_filter: bool,
    /// Ordered-set aggregates: `PERCENTILE_CONT(f) WITHIN GROUP (ORDER BY x)`.
    ///
    /// Standard SQL, and among the built-in dialects PostgreSQL alone. Unlike
    /// most entries here it has no emulation: a median is a property of the
    /// whole sorted group, so no expression over one row can stand in for it,
    /// and a dialect without the clause refuses rather than approximates.
    pub ordered_set_aggregates: bool,
    /// Window functions and the `OVER` clause.
    pub window_functions: bool,
    /// `WINDOW` clause declaring named windows.
    pub named_windows: bool,
    /// `WITH` common table expressions.
    pub cte: bool,
    /// `WITH RECURSIVE`.
    pub recursive_cte: bool,
    /// `RETURNING` on INSERT/UPDATE/DELETE.
    pub returning: bool,
    /// `UPDATE ... FROM <other source>`, so an update may read another table.
    ///
    /// A PostgreSQL extension SQLite adopted in 3.33. MySQL spells the same
    /// thing `UPDATE t1 JOIN t2 SET ...`, which is a different statement shape
    /// rather than a different clause, so it is refused here instead.
    pub update_from: bool,
    /// `DELETE ... USING <other source>`, so a delete may be qualified by
    /// another table.
    ///
    /// PostgreSQL only among the built-in dialects. SQLite has no multi-table
    /// delete at all, and MySQL requires the target table to appear in the
    /// `USING` list as well, which this clause does not express.
    pub delete_using: bool,
    /// `RIGHT JOIN`.
    pub right_join: bool,
    /// `FULL JOIN`.
    pub full_join: bool,
    /// `INTERSECT`.
    pub intersect: bool,
    /// `EXCEPT`.
    pub except: bool,
    /// `ALL` variants of `INTERSECT` / `EXCEPT`.
    pub set_op_all: bool,
    /// Row-locking clauses (`FOR UPDATE`).
    pub row_locking: bool,
    /// `SKIP LOCKED` / `NOWAIT` modifiers on a locking clause.
    pub lock_wait_policy: bool,
    /// `INSERT ... ON CONFLICT` (PostgreSQL, SQLite).
    pub on_conflict: bool,
    /// `INSERT ... ON DUPLICATE KEY UPDATE` (MySQL).
    pub on_duplicate_key: bool,
    /// A single `INSERT` may carry several `VALUES` rows.
    pub multi_row_insert: bool,
    /// Branches of a set operation are wrapped in parentheses.
    pub wrap_set_op_branches: bool,
}

impl Caps {
    /// The capabilities of a modern, standards-following engine. Dialects start
    /// from this and switch off what they lack.
    pub const ANSI: Caps = Caps {
        nulls_ordering: true,
        distinct_on: false,
        aggregate_filter: true,
        ordered_set_aggregates: true,
        window_functions: true,
        named_windows: true,
        cte: true,
        recursive_cte: true,
        returning: false,
        update_from: false,
        delete_using: false,
        right_join: true,
        full_join: true,
        intersect: true,
        except: true,
        set_op_all: true,
        row_locking: true,
        lock_wait_policy: false,
        on_conflict: false,
        on_duplicate_key: false,
        multi_row_insert: true,
        wrap_set_op_branches: true,
    };
}

/// Describes a SQL dialect: its lexical rules, its operator vocabulary, and
/// what it structurally supports.
///
/// Implementing a new dialect means answering the lexical questions, listing
/// the handful of operators that differ from ANSI, and stating the
/// capabilities. Nothing in the renderer changes.
pub trait Dialect {
    /// A stable, human-readable name, used in diagnostics and error messages.
    fn name(&self) -> &'static str;

    /// Quote an identifier, escaping any embedded quote character.
    fn quote_ident(&self, ident: &str) -> String;

    /// Render the placeholder for the Nth (1-based) bound parameter.
    ///
    /// Numbered dialects (PostgreSQL `$N`) use `index`; positional ones
    /// (MySQL, SQLite) ignore it.
    fn placeholder(&self, index: usize) -> String;

    /// The template for an operator, or `None` when this dialect cannot express
    /// it at all.
    fn template(&self, op: Operator) -> Option<Template> {
        templates::ansi(op)
    }

    /// The precedence of an operator, deciding when its arguments need
    /// parentheses.
    fn precedence(&self, op: Operator) -> i16 {
        template::ansi_precedence(op)
    }

    /// What this dialect structurally supports.
    fn caps(&self) -> Caps;

    /// The type name to use as a cast target.
    fn cast_type(&self, kind: CastKind) -> &'static str;

    /// How a boolean literal is spelled when it must be inlined rather than
    /// bound.
    fn bool_literal(&self, value: bool) -> &'static str {
        if value {
            "TRUE"
        } else {
            "FALSE"
        }
    }

    /// A table to select from when a query has no FROM clause, for engines that
    /// require one.
    fn dummy_from(&self) -> Option<&'static str> {
        None
    }

    /// The literal to use as an "unlimited" LIMIT on engines that require a
    /// LIMIT before an OFFSET. `None` means OFFSET stands alone.
    fn unlimited_limit(&self) -> Option<&'static str> {
        None
    }
}

/// Quote an identifier by wrapping it in `quote` and doubling any occurrence of
/// `quote` inside it. Shared by every dialect; only the character differs.
pub(crate) fn quote_with(ident: &str, quote: char) -> String {
    let mut out = String::with_capacity(ident.len() + 2);
    out.push(quote);
    for ch in ident.chars() {
        if ch == quote {
            out.push(quote);
        }
        out.push(ch);
    }
    out.push(quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_quoting_escapes_the_quote_character() {
        assert_eq!(Postgres.quote_ident(r#"we"ird"#), r#""we""ird""#);
        assert_eq!(MySql.quote_ident("we`ird"), "`we``ird`");
        assert_eq!(Sqlite.quote_ident("id"), r#""id""#);
    }

    #[test]
    fn placeholder_styles_differ_by_dialect() {
        assert_eq!(Postgres.placeholder(3), "$3");
        assert_eq!(MySql.placeholder(3), "?");
        assert_eq!(Sqlite.placeholder(3), "?");
    }
}

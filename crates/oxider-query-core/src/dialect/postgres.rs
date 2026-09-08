//! PostgreSQL dialect.

use super::{quote_with, Caps, CastKind, Dialect, Template};
use crate::ast::operator::Operator;
use crate::dialect::template::precedence;
use crate::dialect::templates;

/// PostgreSQL: double-quoted identifiers, numbered `$N` placeholders, and the
/// widest structural support of the three built-in dialects.
#[derive(Debug, Default, Clone, Copy)]
pub struct Postgres;

/// Operators PostgreSQL has no equivalent for.
///
/// A seeded random is the only one: PostgreSQL sets the seed for the session
/// with `setseed` rather than taking it as an argument.
fn lacks(op: Operator) -> bool {
    matches!(op, Operator::RandomSeeded)
}

impl Dialect for Postgres {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn quote_ident(&self, ident: &str) -> String {
        quote_with(ident, '"')
    }

    fn placeholder(&self, index: usize) -> String {
        format!("${index}")
    }

    fn template(&self, op: Operator) -> Option<Template> {
        if lacks(op) {
            return None;
        }
        templates::postgres(op).or_else(|| templates::ansi(op))
    }

    fn precedence(&self, op: Operator) -> i16 {
        // PostgreSQL parses the LIKE family a little more loosely than the
        // ordering comparisons, so it is parenthesised more eagerly.
        // QueryDSL's PostgreSQLTemplates retunes the same entries.
        match op {
            Operator::Like | Operator::LikeEscape | Operator::LikeIc | Operator::LikeEscapeIc => {
                precedence::COMPARISON + 1
            }
            other => super::template::ansi_precedence(other),
        }
    }

    fn caps(&self) -> Caps {
        Caps {
            distinct_on: true,
            returning: true,
            update_from: true,
            delete_using: true,
            lock_wait_policy: true,
            on_conflict: true,
            ..Caps::ANSI
        }
    }

    fn cast_type(&self, kind: CastKind) -> &'static str {
        match kind {
            CastKind::Integer => "BIGINT",
            CastKind::Float => "DOUBLE PRECISION",
            CastKind::Decimal => "NUMERIC",
            CastKind::Text => "TEXT",
            CastKind::Bool => "BOOLEAN",
            CastKind::Date => "DATE",
            CastKind::Time => "TIME",
            CastKind::DateTime => "TIMESTAMP",
        }
    }
}

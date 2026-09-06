//! MySQL and MariaDB dialect.

use super::{quote_with, Caps, CastKind, Dialect, Template};
use crate::ast::operator::Operator;
use crate::dialect::templates;

/// MySQL: backtick-quoted identifiers, positional `?` placeholders.
///
/// The structural gaps worth knowing: no `NULLS FIRST/LAST` (emulated with a
/// leading `CASE WHEN ... IS NULL` sort key), no `FULL JOIN`, no `INTERSECT` or
/// `EXCEPT` before 8.0.31, and no `RETURNING`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MySql;

/// Operators MySQL has no equivalent for.
///
/// Listed explicitly so the ANSI fallback cannot emit SQL MySQL rejects; see
/// the note on SQLite's equivalent list.
fn lacks(op: Operator) -> bool {
    use Operator::*;
    matches!(op, Corr | CovarPop | CovarSamp | NextVal | CurrVal)
}

impl Dialect for MySql {
    fn name(&self) -> &'static str {
        "mysql"
    }

    fn quote_ident(&self, ident: &str) -> String {
        quote_with(ident, '`')
    }

    fn placeholder(&self, _index: usize) -> String {
        "?".to_string()
    }

    fn template(&self, op: Operator) -> Option<Template> {
        if lacks(op) {
            return None;
        }
        templates::mysql(op).or_else(|| templates::ansi(op))
    }

    fn precedence(&self, op: Operator) -> i16 {
        match op {
            // MySQL has a real XOR keyword, which sits between OR and AND
            // rather than among the comparison operators.
            Operator::Xor => crate::dialect::template::precedence::XOR,
            other => crate::dialect::template::ansi_precedence(other),
        }
    }

    fn caps(&self) -> Caps {
        Caps {
            nulls_ordering: false,
            aggregate_filter: false,
            full_join: false,
            intersect: false,
            except: false,
            set_op_all: false,
            on_duplicate_key: true,
            lock_wait_policy: true,
            ..Caps::ANSI
        }
    }

    fn cast_type(&self, kind: CastKind) -> &'static str {
        // MySQL's CAST accepts only a small set of target types, which is why
        // integers become SIGNED and text becomes CHAR rather than the type
        // names the column would use.
        match kind {
            CastKind::Integer => "SIGNED",
            CastKind::Float | CastKind::Decimal => "DECIMAL",
            CastKind::Text => "CHAR",
            CastKind::Bool => "UNSIGNED",
            CastKind::Date => "DATE",
            CastKind::Time => "TIME",
            CastKind::DateTime => "DATETIME",
        }
    }

    fn unlimited_limit(&self) -> Option<&'static str> {
        // MySQL requires a LIMIT before OFFSET; its documented idiom for "no
        // limit" is the largest unsigned 64-bit value.
        Some("18446744073709551615")
    }

    fn bool_literal(&self, value: bool) -> &'static str {
        if value {
            "1"
        } else {
            "0"
        }
    }
}

//! SQLite dialect.

use super::{quote_with, Caps, CastKind, Dialect, Template};
use crate::ast::operator::Operator;
use crate::dialect::templates;

/// SQLite: double-quoted identifiers, positional `?` placeholders.
///
/// SQLite is more capable than its size suggests - it has window functions,
/// recursive CTEs, `RETURNING` (3.35+), `ON CONFLICT`, and `FILTER` on
/// aggregates - but it has no `RIGHT`/`FULL JOIN` before 3.39, no row locking
/// at all (writers take a database-level lock), and no `NULLS FIRST/LAST`
/// before 3.30, so ordering is emulated for portability.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sqlite;

/// Operators SQLite has no equivalent for.
///
/// Listed explicitly rather than left to the ANSI fallback: falling through
/// would emit SQL SQLite cannot parse, and failing here names the operator.
fn lacks(op: Operator) -> bool {
    use Operator::*;
    matches!(
        op,
        StdDev
            | StdDevPop
            | StdDevSamp
            | Variance
            | VarPop
            | VarSamp
            | Corr
            | CovarPop
            | CovarSamp
            | NextVal
            | CurrVal
            | RandomSeeded
            | LPad
            | LPadFill
            | RPad
            | RPadFill
    )
}

impl Dialect for Sqlite {
    fn name(&self) -> &'static str {
        "sqlite"
    }

    fn quote_ident(&self, ident: &str) -> String {
        quote_with(ident, '"')
    }

    fn placeholder(&self, _index: usize) -> String {
        "?".to_string()
    }

    fn template(&self, op: Operator) -> Option<Template> {
        if lacks(op) {
            return None;
        }
        templates::sqlite(op).or_else(|| templates::ansi(op))
    }

    fn caps(&self) -> Caps {
        Caps {
            nulls_ordering: false,
            named_windows: true,
            returning: true,
            update_from: true,
            right_join: false,
            full_join: false,
            set_op_all: false,
            row_locking: false,
            on_conflict: true,
            wrap_set_op_branches: false,
            ..Caps::ANSI
        }
    }

    fn cast_type(&self, kind: CastKind) -> &'static str {
        // SQLite casts to storage classes, not to named SQL types.
        match kind {
            CastKind::Integer | CastKind::Bool => "INTEGER",
            CastKind::Float | CastKind::Decimal => "REAL",
            CastKind::Text | CastKind::Date | CastKind::Time | CastKind::DateTime => "TEXT",
        }
    }

    fn unlimited_limit(&self) -> Option<&'static str> {
        // SQLite requires a LIMIT before OFFSET; a negative limit means none.
        Some("-1")
    }

    fn bool_literal(&self, value: bool) -> &'static str {
        if value {
            "1"
        } else {
            "0"
        }
    }
}

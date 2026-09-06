//! Per-family operator template tables.
//!
//! Each family module holds the ANSI template for its operators plus, side by
//! side, the overrides each dialect needs. Keeping a dialect's divergence next
//! to the baseline is deliberate: the interesting question when reading this
//! code is almost always "how does MySQL differ here", and the answer should be
//! one line away, not in another file.
//!
//! A family function returns `None` for an operator it does not cover, which
//! for a dialect override means "no override, use ANSI" and for [`ansi`] means
//! the operator is not renderable at all on that dialect.

use crate::ast::operator::{Family, Operator};
use crate::dialect::template::Template;

pub(crate) mod agg;
pub(crate) mod core_ops;
pub(crate) mod datetime;
pub(crate) mod math;
pub(crate) mod text;

/// The ANSI template for an operator, used unless a dialect overrides it.
pub fn ansi(op: Operator) -> Option<Template> {
    match op.family() {
        Family::Comparison | Family::Boolean | Family::Conditional => core_ops::ansi(op),
        Family::Math => math::ansi(op),
        Family::Text => text::ansi(op),
        Family::DateTime => datetime::ansi(op),
        Family::Aggregate | Family::Window | Family::Sequence => agg::ansi(op),
    }
}

/// PostgreSQL's overrides.
pub fn postgres(op: Operator) -> Option<Template> {
    match op.family() {
        Family::Comparison | Family::Boolean | Family::Conditional => core_ops::postgres(op),
        Family::Math => math::postgres(op),
        Family::Text => text::postgres(op),
        Family::DateTime => datetime::postgres(op),
        Family::Aggregate | Family::Window | Family::Sequence => agg::postgres(op),
    }
}

/// MySQL's overrides.
pub fn mysql(op: Operator) -> Option<Template> {
    match op.family() {
        Family::Comparison | Family::Boolean | Family::Conditional => core_ops::mysql(op),
        Family::Math => math::mysql(op),
        Family::Text => text::mysql(op),
        Family::DateTime => datetime::mysql(op),
        Family::Aggregate | Family::Window | Family::Sequence => agg::mysql(op),
    }
}

/// SQLite's overrides.
pub fn sqlite(op: Operator) -> Option<Template> {
    match op.family() {
        Family::Comparison | Family::Boolean | Family::Conditional => core_ops::sqlite(op),
        Family::Math => math::sqlite(op),
        Family::Text => text::sqlite(op),
        Family::DateTime => datetime::sqlite(op),
        Family::Aggregate | Family::Window | Family::Sequence => agg::sqlite(op),
    }
}

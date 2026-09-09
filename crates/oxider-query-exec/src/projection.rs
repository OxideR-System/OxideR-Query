//! Reading a result row by position rather than by column name.
//!
//! sqlx's own `FromRow` matches fields to columns by name, which needs every
//! selected expression to carry an alias and reports a mismatch only once a row
//! comes back. A [`Projection`] instead reads a fixed-width **span** of columns
//! starting at an offset, so it matches the order of the `select` list.
//!
//! The offset is what makes spans compose. A tuple projection hands its first
//! element the offset it was given and its second element that offset plus the
//! first element's [`Projection::ARITY`], so
//!
//! ```ignore
//! let rows: Vec<(User, Option<Order>)> = db.fetch_all_projected(query).await?;
//! ```
//!
//! reads one flat join into two structs, with the widths added up at compile
//! time. That is the shape [`group_children`](crate::group_children) folds.
//!
//! What is *not* checked at compile time is whether the span widths add up to
//! the query's own projection: `Select` erases its projection into a `Vec<Node>`
//! as soon as it is built, so there is no type left to compare against. A
//! mismatch surfaces as a column-index error from the first row rather than as a
//! compile error. Widening `Select` to carry its output types would thread a
//! fourth type parameter through every method on it, which is a bigger change
//! than the guarantee is worth today.

use sqlx::{Error, Row, ValueRef};

/// A value read from a fixed-width span of a result row.
///
/// Implemented by `#[derive(Projection)]` for structs, and below for tuples and
/// `Option`. `'r` borrows the row and `R` is the row type, so one projection
/// works across every backend whose columns can decode its fields.
pub trait Projection<'r, R: Row>: Sized {
    /// How many columns this projection consumes.
    ///
    /// A struct's arity is its field count; a tuple's is the sum of its
    /// elements'; an `Option`'s is that of the projection inside it.
    const ARITY: usize;

    /// Read this value from the columns starting at `offset`.
    fn from_row_at(row: &'r R, offset: usize) -> Result<Self, Error>;
}

/// An optional projection, for the outer half of a `LEFT JOIN`.
///
/// `None` when **every** column in the span is NULL, which is what a join that
/// matched nothing produces. A row where only some of the span is NULL is a
/// real row with nullable fields, so it decodes as `Some` and any `NOT NULL`
/// field inside it fails the way it would on its own.
impl<'r, R, P> Projection<'r, R> for Option<P>
where
    R: Row,
    P: Projection<'r, R>,
    usize: sqlx::ColumnIndex<R>,
{
    const ARITY: usize = P::ARITY;

    fn from_row_at(row: &'r R, offset: usize) -> Result<Self, Error> {
        for index in offset..offset + P::ARITY {
            if !row.try_get_raw(index)?.is_null() {
                return P::from_row_at(row, offset).map(Some);
            }
        }
        Ok(None)
    }
}

/// Generate a tuple projection impl of the given arity.
///
/// Each element starts where the previous one ended, so the offsets are a
/// running sum of the elements' widths - computed in the type system, not from
/// the row.
macro_rules! tuple_projection {
    ($head:ident $(, $tail:ident)* $(,)?) => {
        #[allow(non_snake_case, unused_assignments)]
        impl<'r, R, $head $(, $tail)*> Projection<'r, R> for ($head, $($tail,)*)
        where
            R: Row,
            $head: Projection<'r, R>,
            $($tail: Projection<'r, R>,)*
        {
            const ARITY: usize = $head::ARITY $(+ $tail::ARITY)*;

            fn from_row_at(row: &'r R, offset: usize) -> Result<Self, Error> {
                let mut at = offset;
                let $head = $head::from_row_at(row, at)?;
                at += $head::ARITY;
                $(
                    let $tail = $tail::from_row_at(row, at)?;
                    at += $tail::ARITY;
                )*
                Ok(($head, $($tail,)*))
            }
        }
    };
}

tuple_projection!(A, B);
tuple_projection!(A, B, C);
tuple_projection!(A, B, C, D);
tuple_projection!(A, B, C, D, E);
tuple_projection!(A, B, C, D, E, F);
tuple_projection!(A, B, C, D, E, F, G);
tuple_projection!(A, B, C, D, E, F, G, H);

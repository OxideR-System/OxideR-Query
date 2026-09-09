//! Type-level source sets.
//!
//! A query tracks, purely at the type level, which entities are in scope: the
//! FROM entity plus every joined entity. Expressions carry the set of entities
//! they reference. `filter`/`select`/`order_by` then require the referenced set
//! to be contained in the in-scope set, so referencing a column of an entity you
//! forgot to join is a compile error.
//!
//! Sets are cons-lists (`Cons`/`Nil`). Membership uses a witness index type
//! (`Here`/`There`), the frunk trick that keeps the two `Contains` impls from
//! overlapping; the witness is inferred by the compiler and never named by
//! users.

use core::marker::PhantomData;

/// The empty source list.
pub struct Nil;

/// A cons cell: entity `Head` followed by the rest of the list `Tail`.
pub struct Cons<Head, Tail>(PhantomData<fn() -> (Head, Tail)>);

/// Witness that the searched entity is at the head of the current cell.
pub struct Here;

/// Witness that the searched entity is somewhere in the tail.
pub struct There<Idx>(PhantomData<Idx>);

/// `Self` contains entity `E`, at the position witnessed by `Idx`.
///
/// Unsatisfied, this is the error for reading a table the query never brought
/// into scope. The compiler would otherwise report it as `Nil: Contains<User,
/// _>`, naming the type-level list rather than the mistake, so the message is
/// written out here instead.
#[diagnostic::on_unimplemented(
    message = "`{E}` is not in scope in this query",
    label = "this reads a column of `{E}`",
    note = "a query may only read the entities it selects from and joins to",
    note = "join `{E}`, start the query from it, or move this into a subquery over it"
)]
pub trait Contains<E, Idx> {}

impl<E, Tail> Contains<E, Here> for Cons<E, Tail> {}

impl<E, Head, Tail, Idx> Contains<E, There<Idx>> for Cons<Head, Tail> where Tail: Contains<E, Idx> {}

/// Every entity in `List` is contained in `Self`, witnessed by `Idxs`.
#[diagnostic::on_unimplemented(
    message = "this reads entities the query does not have in scope",
    label = "one of these entities is not in the query",
    note = "the query has `{Self}` in scope and this expression needs `{List}`",
    note = "join what is missing, or move this into a subquery over it"
)]
pub trait ContainsAll<List, Idxs> {}

impl<S> ContainsAll<Nil, Nil> for S {}

impl<S, Head, Tail, IdxHead, IdxTail> ContainsAll<Cons<Head, Tail>, Cons<IdxHead, IdxTail>> for S
where
    S: Contains<Head, IdxHead>,
    S: ContainsAll<Tail, IdxTail>,
{
}

/// Type-level concatenation of two source lists.
pub trait Concat<Rhs> {
    /// The concatenated list.
    type Out;
}

impl<Rhs> Concat<Rhs> for Nil {
    type Out = Rhs;
}

impl<Head, Tail, Rhs> Concat<Rhs> for Cons<Head, Tail>
where
    Tail: Concat<Rhs>,
{
    type Out = Cons<Head, <Tail as Concat<Rhs>>::Out>;
}

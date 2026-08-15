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
pub trait Contains<E, Idx> {}

impl<E, Tail> Contains<E, Here> for Cons<E, Tail> {}

impl<E, Head, Tail, Idx> Contains<E, There<Idx>> for Cons<Head, Tail> where Tail: Contains<E, Idx> {}

/// Every entity in `List` is contained in `Self`, witnessed by `Idxs`.
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

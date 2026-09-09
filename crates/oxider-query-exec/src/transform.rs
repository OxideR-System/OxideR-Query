//! Folding a flat join result back into the shape it describes.
//!
//! A one-to-many join returns the parent repeated once per child. QueryDSL
//! solves that with `transform(groupBy(...))`; this is the same idea with the
//! parts Rust already has, so it is one function rather than a builder.

use core::hash::Hash;
use std::collections::HashMap;

/// Group a flat `(parent, child)` result into one entry per parent.
///
/// Rows arrive as a query returned them - the parent repeated once per child,
/// with a `None` child where a `LEFT JOIN` matched nothing - and come back with
/// one entry per distinct key, holding every child seen under it.
///
/// ```no_run
/// # async fn demo() -> oxider_query_exec::Result<()> {
/// # use oxider_query::prelude::*;
/// # use oxider_query_exec::{group_children, SqliteDb};
/// # #[derive(Entity, oxider_query::Projection)]
/// # #[oxider(table = "users")] struct User { id: i64, name: String }
/// # #[derive(Entity, oxider_query::Projection)]
/// # #[oxider(table = "orders")] struct Order { id: i64, user_id: i64 }
/// # let db: SqliteDb = todo!();
/// let query = User::query()
///     .left_join(Order::table(), Order::user_id.eq(User::id))
///     .select((User::id, User::name, Order::id, Order::user_id))
///     .order_by(User::id.asc());
///
/// let rows: Vec<(User, Option<Order>)> = db.fetch_all_projected(query).await?;
/// let tree: Vec<(User, Vec<Order>)> = group_children(rows, |user| user.id);
/// # Ok(())
/// # }
/// ```
///
/// # Order
///
/// Parents come back in the order they first appeared, and each parent's
/// children in the order they arrived. A `HashMap` would have thrown away the
/// `ORDER BY` the query went to the trouble of asking for, so the map here holds
/// only positions into the result and the result itself stays a `Vec`.
///
/// # The parent kept
///
/// The first row wins. Every row under one key carries the same parent columns,
/// so later copies are duplicates; keeping the first avoids cloning to compare
/// them and avoids requiring `PartialEq` on the parent.
pub fn group_children<P, C, K>(
    rows: impl IntoIterator<Item = (P, Option<C>)>,
    key: impl Fn(&P) -> K,
) -> Vec<(P, Vec<C>)>
where
    K: Eq + Hash,
{
    let rows = rows.into_iter();
    let (lower, _) = rows.size_hint();
    let mut grouped: Vec<(P, Vec<C>)> = Vec::with_capacity(lower);
    let mut position: HashMap<K, usize> = HashMap::with_capacity(lower);

    for (parent, child) in rows {
        let at = match position.entry(key(&parent)) {
            std::collections::hash_map::Entry::Occupied(slot) => *slot.get(),
            std::collections::hash_map::Entry::Vacant(slot) => {
                let at = grouped.len();
                slot.insert(at);
                grouped.push((parent, Vec::new()));
                at
            }
        };
        if let Some(child) = child {
            grouped[at].1.push(child);
        }
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::group_children;

    #[derive(Debug, PartialEq)]
    struct User {
        id: i64,
    }

    #[test]
    fn children_collect_under_one_parent() {
        let rows = vec![
            (User { id: 1 }, Some("a")),
            (User { id: 1 }, Some("b")),
            (User { id: 2 }, Some("c")),
        ];
        assert_eq!(
            group_children(rows, |user| user.id),
            vec![
                (User { id: 1 }, vec!["a", "b"]),
                (User { id: 2 }, vec!["c"]),
            ]
        );
    }

    #[test]
    fn a_parent_with_no_children_keeps_an_empty_vec() {
        let rows = vec![(User { id: 1 }, None::<&str>), (User { id: 2 }, Some("c"))];
        assert_eq!(
            group_children(rows, |user| user.id),
            vec![(User { id: 1 }, vec![]), (User { id: 2 }, vec!["c"])]
        );
    }

    /// The reason this returns a `Vec` rather than a map: the query asked for an
    /// order, and the fold must not throw it away.
    #[test]
    fn parents_stay_in_the_order_they_first_appeared() {
        let rows = vec![
            (User { id: 30 }, Some("a")),
            (User { id: 10 }, Some("b")),
            (User { id: 20 }, Some("c")),
            (User { id: 10 }, Some("d")),
        ];
        let grouped = group_children(rows, |user| user.id);
        assert_eq!(
            grouped.iter().map(|(u, _)| u.id).collect::<Vec<_>>(),
            vec![30, 10, 20]
        );
        assert_eq!(grouped[1].1, vec!["b", "d"], "and so do the children");
    }

    /// Rows for one parent do not have to be adjacent, so the fold cannot lean
    /// on a run-length pass over sorted input.
    #[test]
    fn interleaved_parents_still_group() {
        let rows = vec![
            (User { id: 1 }, Some("a")),
            (User { id: 2 }, Some("b")),
            (User { id: 1 }, Some("c")),
        ];
        assert_eq!(
            group_children(rows, |user| user.id),
            vec![
                (User { id: 1 }, vec!["a", "c"]),
                (User { id: 2 }, vec!["b"])
            ]
        );
    }
}

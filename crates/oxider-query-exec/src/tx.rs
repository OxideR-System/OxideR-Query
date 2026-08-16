//! The [`Tx`] handle: a transaction exposing the same query methods as [`Db`].

use crate::{ops, Backend};
use oxider_query_core::Renderable;
use sqlx::{Database, FromRow, Transaction};

/// An in-progress transaction, obtained from [`Db::begin`](crate::Db::begin).
///
/// Exposes the same query methods as [`Db`](crate::Db) - [`execute`](Tx::execute),
/// [`fetch_all`](Tx::fetch_all), [`fetch_one`](Tx::fetch_one),
/// [`fetch_optional`](Tx::fetch_optional) - running each on the transaction.
/// Finish with [`commit`](Tx::commit) or [`rollback`](Tx::rollback); dropping the
/// handle without committing rolls back.
pub struct Tx<DB: Backend> {
    tx: Transaction<'static, DB>,
}

impl<DB: Backend> Tx<DB> {
    /// Wrap a sqlx transaction. Called by [`Db::begin`](crate::Db::begin).
    pub(crate) fn new(tx: Transaction<'static, DB>) -> Self {
        Self { tx }
    }

    /// Commit the transaction, making its changes durable.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }

    /// Roll the transaction back, discarding its changes.
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }
}

impl<DB: Backend> Tx<DB>
where
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'e> &'e mut <DB as Database>::Connection: sqlx::Executor<'e, Database = DB>,
{
    /// Run a statement (typically INSERT/UPDATE/DELETE) and return the number of
    /// affected rows.
    pub async fn execute<Q>(&mut self, query: Q) -> Result<u64, sqlx::Error>
    where
        Q: Renderable,
    {
        ops::execute(&mut *self.tx, query).await
    }

    /// Run a query and collect every row into `O`.
    pub async fn fetch_all<O, Q>(&mut self, query: Q) -> Result<Vec<O>, sqlx::Error>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_all(&mut *self.tx, query).await
    }

    /// Run a query expected to return exactly one row.
    pub async fn fetch_one<O, Q>(&mut self, query: Q) -> Result<O, sqlx::Error>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_one(&mut *self.tx, query).await
    }

    /// Run a query that may return zero or one row.
    pub async fn fetch_optional<O, Q>(&mut self, query: Q) -> Result<Option<O>, sqlx::Error>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_optional(&mut *self.tx, query).await
    }
}

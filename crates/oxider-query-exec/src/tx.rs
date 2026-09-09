//! The [`Tx`] handle: a transaction exposing the same query methods as [`Db`].

use crate::{ops, Backend, Page, Projection, Result};
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
    pub async fn commit(self) -> Result<()> {
        Ok(self.tx.commit().await?)
    }

    /// Roll the transaction back, discarding its changes.
    pub async fn rollback(self) -> Result<()> {
        Ok(self.tx.rollback().await?)
    }
}

impl<DB: Backend> Tx<DB>
where
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'e> &'e mut <DB as Database>::Connection: sqlx::Executor<'e, Database = DB>,
{
    /// Run a statement (typically INSERT/UPDATE/DELETE) and return the number of
    /// affected rows.
    pub async fn execute<Q>(&mut self, query: Q) -> Result<u64>
    where
        Q: Renderable,
    {
        ops::execute(&mut *self.tx, query).await
    }

    /// Run a query and collect every row into `O`.
    pub async fn fetch_all<O, Q>(&mut self, query: Q) -> Result<Vec<O>>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_all(&mut *self.tx, query).await
    }

    /// Run a query expected to return exactly one row.
    pub async fn fetch_one<O, Q>(&mut self, query: Q) -> Result<O>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_one(&mut *self.tx, query).await
    }

    /// Run a query that may return zero or one row.
    pub async fn fetch_optional<O, Q>(&mut self, query: Q) -> Result<Option<O>>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_optional(&mut *self.tx, query).await
    }

    /// Run a query and read every row by column position into `O`.
    ///
    /// The positional counterpart of [`fetch_all`](Tx::fetch_all); see
    /// [`Projection`].
    pub async fn fetch_all_projected<O, Q>(&mut self, query: Q) -> Result<Vec<O>>
    where
        O: for<'r> Projection<'r, DB::Row>,
        Q: Renderable,
    {
        ops::fetch_all_projected(&mut *self.tx, query).await
    }

    /// Run a query expected to return exactly one row, read by column position.
    pub async fn fetch_one_projected<O, Q>(&mut self, query: Q) -> Result<O>
    where
        O: for<'r> Projection<'r, DB::Row>,
        Q: Renderable,
    {
        ops::fetch_one_projected(&mut *self.tx, query).await
    }

    /// Run a query that may return zero or one row, read by column position.
    pub async fn fetch_optional_projected<O, Q>(&mut self, query: Q) -> Result<Option<O>>
    where
        O: for<'r> Projection<'r, DB::Row>,
        Q: Renderable,
    {
        ops::fetch_optional_projected(&mut *self.tx, query).await
    }

    /// Count the rows a query returns, ignoring any `LIMIT` and `OFFSET` on it.
    ///
    /// Wraps the query rather than swapping its projection for `COUNT(*)`, so
    /// the answer is right for `DISTINCT`, `GROUP BY` and set operations too.
    /// See [`Select::count`](oxider_query_core::Select::count).
    pub async fn fetch_count<S, F, L>(
        &mut self,
        query: ::oxider_query_core::Select<S, F, L>,
    ) -> Result<u64>
    where
        for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        usize: sqlx::ColumnIndex<DB::Row>,
    {
        ops::fetch_count(&mut *self.tx, query.count()).await
    }

    /// Fetch one page of a query, and the total it was taken from.
    ///
    /// Counts first, then re-runs the query with `LIMIT`/`OFFSET` applied, so
    /// the two answers describe the same query rather than two hand-written ones
    /// that have to be kept in step. Pages are numbered from zero.
    ///
    /// Two round trips, deliberately: a windowed `COUNT(*) OVER ()` would do it
    /// in one but returns nothing at all when the page is past the end, which is
    /// exactly when the caller most needs the total.
    pub async fn fetch_page<O, S, F, L>(
        &mut self,
        query: ::oxider_query_core::Select<S, F, L>,
        number: u64,
        size: u64,
    ) -> Result<Page<O>>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        usize: sqlx::ColumnIndex<DB::Row>,
    {
        let total = self.fetch_count(query.clone()).await?;
        let items = self.fetch_all(query.page(number, size)).await?;
        Ok(Page {
            items,
            total,
            size,
            number,
        })
    }

    /// Fetch one page, reading each row by column position.
    ///
    /// The positional counterpart of [`fetch_page`](Tx::fetch_page).
    pub async fn fetch_page_projected<O, S, F, L>(
        &mut self,
        query: ::oxider_query_core::Select<S, F, L>,
        number: u64,
        size: u64,
    ) -> Result<Page<O>>
    where
        O: for<'r> Projection<'r, DB::Row>,
        for<'r> i64: sqlx::Decode<'r, DB> + sqlx::Type<DB>,
        usize: sqlx::ColumnIndex<DB::Row>,
    {
        let total = self.fetch_count(query.clone()).await?;
        let items = self.fetch_all_projected(query.page(number, size)).await?;
        Ok(Page {
            items,
            total,
            size,
            number,
        })
    }
}

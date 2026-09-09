//! The [`Db`] handle: a sqlx pool paired with its backend's dialect.

use crate::{ops, Backend, Projection, Result, Tx};
use oxider_query_core::Renderable;
use sqlx::{Database, FromRow, Pool};

/// A database handle: a sqlx pool paired with its backend's dialect.
///
/// Construct one with [`Db::connect`] or [`Db::new`], then run queries with
/// [`execute`](Db::execute), [`fetch_all`](Db::fetch_all),
/// [`fetch_one`](Db::fetch_one) or [`fetch_optional`](Db::fetch_optional). The
/// same handle runs SELECTs and mutations; the dialect is never named at the
/// call site. [`begin`](Db::begin) starts a transaction.
pub struct Db<DB: Backend> {
    pool: Pool<DB>,
}

impl<DB: Backend> Db<DB>
where
    // sqlx needs the backend's arguments to be convertible for `execute`/`fetch`,
    // and a pool reference to be an executor. Both hold for every sqlx database;
    // stating them once here keeps the method signatures clean.
    for<'q> <DB as Database>::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
{
    /// Wrap an existing sqlx pool.
    pub fn new(pool: Pool<DB>) -> Self {
        Self { pool }
    }

    /// Open a pool from a connection URL.
    pub async fn connect(url: &str) -> Result<Self> {
        Ok(Self {
            pool: Pool::connect(url).await?,
        })
    }

    /// Borrow the underlying sqlx pool (for raw sqlx access).
    pub fn pool(&self) -> &Pool<DB> {
        &self.pool
    }

    /// Begin a transaction, returning a [`Tx`] handle with the same query
    /// methods. Finish it with [`Tx::commit`] or [`Tx::rollback`]; dropping it
    /// without committing rolls back.
    ///
    /// For the common commit-on-success, rollback-on-error pattern, prefer the
    /// scoped [`transaction`](Db::transaction) helper, which cannot forget to
    /// commit.
    pub async fn begin(&self) -> Result<Tx<DB>> {
        Ok(Tx::new(self.pool.begin().await?))
    }

    /// Run `f` inside a transaction, committing if it returns `Ok` and rolling
    /// back if it returns `Err`. The closure receives the [`Tx`] handle and runs
    /// its queries on it; whatever it returns is returned here.
    ///
    /// This is the scoped analogue of Spring's `@Transactional`: the transaction
    /// boundary is the closure, so a commit can never be forgotten and any early
    /// return or error rolls back. The error type only needs to be convertible
    /// from [`Error`](crate::Error) (for the begin/commit/rollback steps), so a
    /// closure returning [`crate::Result`] works directly.
    ///
    /// ```no_run
    /// # async fn demo(db: &oxider_query_exec::SqliteDb) -> oxider_query_exec::Result<()> {
    /// # use oxider_query::prelude::*;
    /// # #[derive(Entity)] #[oxider(table = "users")] struct User { id: i64, age: i64 }
    /// db.transaction(async |tx| {
    ///     tx.execute(User::insert().set(User::id, 1).set(User::age, 30)).await?;
    ///     tx.execute(User::update().set(User::age, 31).filter(User::id.eq(1))).await?;
    ///     Ok(())
    /// })
    /// .await
    /// # }
    /// ```
    pub async fn transaction<F, T, E>(&self, f: F) -> core::result::Result<T, E>
    where
        F: AsyncFnOnce(&mut Tx<DB>) -> core::result::Result<T, E>,
        E: From<crate::Error>,
    {
        let mut tx = self.begin().await?;
        match f(&mut tx).await {
            Ok(value) => {
                tx.commit().await?;
                Ok(value)
            }
            Err(err) => {
                // The caller's error is the one worth reporting. A rollback
                // that fails almost always fails because the connection is
                // already broken - which is usually why the statement failed in
                // the first place - so propagating the rollback error instead
                // would replace the cause with a symptom. Dropping the handle
                // rolls back too, so nothing stays open either way.
                let _ = tx.rollback().await;
                Err(err)
            }
        }
    }

    /// Run a statement (typically INSERT/UPDATE/DELETE) and return the number of
    /// affected rows.
    pub async fn execute<Q>(&self, query: Q) -> Result<u64>
    where
        Q: Renderable,
    {
        ops::execute(&self.pool, query).await
    }

    /// Run a query and collect every row into `O`.
    pub async fn fetch_all<O, Q>(&self, query: Q) -> Result<Vec<O>>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_all(&self.pool, query).await
    }

    /// Run a query expected to return exactly one row.
    pub async fn fetch_one<O, Q>(&self, query: Q) -> Result<O>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_one(&self.pool, query).await
    }

    /// Run a query that may return zero or one row.
    pub async fn fetch_optional<O, Q>(&self, query: Q) -> Result<Option<O>>
    where
        O: for<'r> FromRow<'r, DB::Row> + Send + Unpin,
        Q: Renderable,
    {
        ops::fetch_optional(&self.pool, query).await
    }

    /// Run a query and read every row by column position into `O`.
    ///
    /// The positional counterpart of [`fetch_all`](Db::fetch_all): `O` is a
    /// [`Projection`] rather than a `FromRow`, so it matches the order of the
    /// `select` list instead of column names, and tuples of projections read one
    /// flat join into several structs.
    pub async fn fetch_all_projected<O, Q>(&self, query: Q) -> Result<Vec<O>>
    where
        O: for<'r> Projection<'r, DB::Row>,
        Q: Renderable,
    {
        ops::fetch_all_projected(&self.pool, query).await
    }

    /// Run a query expected to return exactly one row, read by column position.
    pub async fn fetch_one_projected<O, Q>(&self, query: Q) -> Result<O>
    where
        O: for<'r> Projection<'r, DB::Row>,
        Q: Renderable,
    {
        ops::fetch_one_projected(&self.pool, query).await
    }

    /// Run a query that may return zero or one row, read by column position.
    pub async fn fetch_optional_projected<O, Q>(&self, query: Q) -> Result<Option<O>>
    where
        O: for<'r> Projection<'r, DB::Row>,
        Q: Renderable,
    {
        ops::fetch_optional_projected(&self.pool, query).await
    }
}

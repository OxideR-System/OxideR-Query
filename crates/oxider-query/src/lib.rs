//! `oxider-query`: type-safe, multi-dialect SQL query builder for Rust.
//!
//! Inspired by Java's QueryDSL, but pushing type-safety further than a JVM can:
//! define entities as plain structs, derive [`Entity`], and the compiler checks
//! not only that a column's type matches what you compare it to, but that the
//! table it belongs to is actually in the query.
//!
//! ```
//! use oxider_query::prelude::*;
//!
//! #[derive(Entity)]
//! #[oxider(table = "users")]
//! struct User {
//!     id: i64,
//!     name: String,
//!     age: i32,
//!     email: Option<String>,
//! }
//!
//! let rendered = User::query()
//!     .select((User::id, User::name))
//!     .filter(User::name.contains("nguyen").and(User::age.ge(18)))
//!     .order_by(User::id.desc())
//!     .limit(20)
//!     .to_sql(&Postgres)
//!     .unwrap();
//!
//! assert!(rendered.sql.starts_with(r#"SELECT "users"."id", "users"."name" FROM "users""#));
//! assert_eq!(rendered.params.len(), 2);
//! ```
//!
//! # What the compiler catches
//!
//! Comparing a column against the wrong type:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! User::name.eq(123); // `i64` is not an expression of type `String`
//! ```
//!
//! Referencing a table the query never joined:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! # #[derive(Entity)] #[oxider(table = "departments")]
//! # struct Department { id: i64, name: String }
//! // Department was never joined -> `Nil: Contains<Department>` is unsatisfied.
//! User::query().filter(Department::name.eq("AI"));
//! ```
//!
//! Aggregating a column the aggregate does not apply to:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "orders")]
//! # struct Order { id: i64, status: String }
//! Order::status.sum(); // `String: Numeric` is unsatisfied
//! ```
//!
//! Assigning a column of a different entity in an UPDATE:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! # #[derive(Entity)] #[oxider(table = "departments")]
//! # struct Department { id: i64, name: String }
//! User::update().set(Department::name, "x");
//! ```
//!
//! Selecting a field the entity skipped:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! #[derive(Entity)]
//! #[oxider(table = "people")]
//! struct Person {
//!     id: i64,
//!     #[oxider(skip)]
//!     display: String,
//! }
//! Person::query().select(Person::display); // no such associated const
//! ```
//!
//! An `IN` subquery selecting the wrong type:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, department_id: i64 }
//! # #[derive(Entity)] #[oxider(table = "departments")]
//! # struct Department { id: i64, name: String }
//! // department_id is i64 but the subquery selects a String column.
//! let sub = Department::query().scalar(Department::name);
//! User::query().filter(User::department_id.in_subquery(sub));
//! ```
//!
//! A lock wait policy with no lock to modify. This one is not a typo but a
//! silent wrong answer: it used to render a query with no locking at all, so a
//! queue worker asking to skip locked rows quietly processed rows another
//! worker already held.
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "jobs")]
//! # struct Job { id: i64 }
//! Job::query().skip_locked(); // `skip_locked` exists only on a locked query
//! ```
//!
//! ```
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "jobs")]
//! # struct Job { id: i64 }
//! // With the lock, it is the queue-worker query it was meant to be.
//! let sql = Job::query().limit(1).for_update().skip_locked().to_sql(&Postgres).unwrap().sql;
//! assert!(sql.ends_with("FOR UPDATE SKIP LOCKED"));
//! ```
//!
//! An `EXCLUDE` on a window that has no frame to exclude from:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "posts")]
//! # struct Post { id: i64, views: i64 }
//! Window::new().exclude(FrameExclusion::CurrentRow); // no frame was set
//! ```
//!
//! Mixing the three INSERT spellings, each of which would have discarded part
//! of what was already written:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! // `columns` would reset the row `set` just built.
//! User::insert().set(User::name, "ada").columns((User::id,));
//! ```
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! // The query supplies the rows, so the value here had nowhere to go.
//! User::insert().from_query(User::query().select(User::name)).set(User::name, "ada");
//! ```
//!
//! A table name built at runtime. Identifiers are `&'static str` precisely so
//! that a string an application assembled cannot become one, which is what
//! keeps identifier injection off the table; values go through the bind list
//! instead.
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! let untrusted = "users";
//! let name = format!("t_{untrusted}");
//! select_from_name(&name); // `name` does not live long enough
//! ```
//!
//! Ordering a type that has no order:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, active: bool }
//! User::active.gt(true); // `bool: Orderable` is unsatisfied
//! ```
//!
//! An INSERT row that does not match the column list:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String, age: i32 }
//! // `age` is an i32 column, so a string is not a value for it.
//! User::insert().columns((User::name, User::age)).values(("ada", "36"));
//! ```
//!
//! Using a correlated subquery where the table it correlates to is not in
//! scope. This is what the second type parameter of `Select` exists for:
//!
//! ```compile_fail
//! # use oxider_query::prelude::*;
//! # #[derive(Entity)] #[oxider(table = "users")]
//! # struct User { id: i64, name: String }
//! # #[derive(Entity)] #[oxider(table = "orders")]
//! # struct Order { id: i64, user_id: i64, total: f64 }
//! # #[derive(Entity)] #[oxider(table = "order_items")]
//! # struct OrderItem { id: i64, order_id: i64, price: f64 }
//! let item_total = OrderItem::query()
//!     .correlate::<Order>()
//!     .filter(OrderItem::order_id.eq(Order::id))
//!     .scalar(OrderItem::price.sum());
//! // The subquery is free in `Order`, which this query never joined.
//! User::query().select((User::name, item_total));
//! ```
//!
//! This facade re-exports the core crate and the derive macro, so downstream
//! crates depend only on `oxider-query`.

pub use oxider_query_core::*;
pub use oxider_query_macros::Entity;

/// Everything needed to write queries, in one import.
///
/// The operator traits have to be in scope for their methods to resolve, which
/// is why this exists rather than a handful of individual imports.
pub mod prelude {
    pub use oxider_query_core::prelude::*;
    pub use oxider_query_macros::Entity;
}

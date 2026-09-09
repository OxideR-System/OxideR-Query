//! Derive macro generating the query metamodel for an entity struct.
//!
//! `#[derive(Entity)]` implements `Entity` for the struct and generates one
//! associated-const `Column` per field, so queries read as `User::id`. Generated
//! code references the facade crate as `::oxider_query`, so users only depend on
//! `oxider-query`.
//!
//! QueryDSL solves the same problem with an annotation processor that writes a
//! `QUser` class to a generated source directory. A derive macro reaches the
//! same place without a build step, without a second name to import, and
//! without generated files in the tree.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attributes;
mod entity;
mod projection;

/// Derive the query metamodel for a struct.
///
/// ```ignore
/// #[derive(Entity)]
/// #[oxider(table = "users", schema = "app")]
/// struct User {
///     id: i64,
///     #[oxider(column = "full_name")]
///     name: String,
///     email: Option<String>,
/// }
/// ```
///
/// Attributes:
///
/// - `#[oxider(table = "...")]` on the struct: the table name. Defaults to the
///   lowercased struct name.
/// - `#[oxider(schema = "...")]` on the struct: the schema qualifier.
/// - `#[oxider(column = "...")]` on a field: the column name. Defaults to the
///   field name.
/// - `#[oxider(skip)]` on a field: leave it out of the metamodel, for fields
///   that are not columns.
#[proc_macro_derive(Entity, attributes(oxider))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match entity::expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive positional row reading for a struct.
///
/// ```ignore
/// #[derive(Entity, Projection)]
/// #[oxider(table = "users")]
/// struct User {
///     id: i64,
///     name: String,
/// }
///
/// let rows: Vec<(User, Option<Order>)> = db.fetch_all_projected(query).await?;
/// ```
///
/// Field *n* reads column *n* of the struct's span, so the struct follows the
/// order of the `select` list rather than column names. Tuples of projections
/// split one flat join into several structs, which is the shape
/// `group_children` folds.
///
/// The generated impl names `oxider-query-exec`, so that crate has to be a
/// dependency; it is what turns a row into a value, and it is already there for
/// anyone running queries.
///
/// `#[oxider(skip)]` fields consume no column and are filled with
/// `Default::default()`, which is the only value available for something the row
/// does not carry.
#[proc_macro_derive(Projection, attributes(oxider))]
pub fn derive_projection(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match projection::expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

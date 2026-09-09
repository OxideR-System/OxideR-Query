//! `#[derive(Projection)]`: read a struct from a span of result columns by
//! position.
//!
//! The generated impl reads field *n* from column `offset + n`, so it follows
//! the order of the `select` list rather than column names. That is what lets a
//! tuple of projections split one flat join into several structs, and what frees
//! the query from having to alias every expression.
//!
//! Generated code names `::oxider_query_exec`, not the facade. A projection is
//! only meaningful when something is executing the query, so the crate that does
//! the executing is the honest dependency; someone who renders SQL and binds it
//! with their own driver has no use for this derive.

use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::attributes::ColumnOptions;

pub(crate) fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "Projection requires a struct with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                ident,
                "Projection can only be derived on structs",
            ))
        }
    };

    // Two lists rather than one: `reads` consumes columns and so decides the
    // arity, while `skipped` fills fields that are not columns at all.
    let mut reads = Vec::new();
    let mut skipped = Vec::new();
    let mut decode_bounds = Vec::new();
    let mut arity = 0usize;

    for field in fields {
        let fid = field.ident.as_ref().expect("named field");
        let ty = &field.ty;
        if ColumnOptions::parse(field)?.skip {
            // A skipped field is not in the row, so there is nothing to read it
            // from. `Default` is the only value that can be produced without
            // inventing one.
            skipped.push(quote! { #fid: ::core::default::Default::default() });
            continue;
        }
        let index = arity;
        reads.push(quote! {
            #fid: ::sqlx::Row::try_get(row, offset + #index)?
        });
        decode_bounds.push(quote! {
            #ty: ::sqlx::Decode<'r, <R as ::sqlx::Row>::Database>
                + ::sqlx::Type<<R as ::sqlx::Row>::Database>
        });
        arity += 1;
    }

    if arity == 0 {
        return Err(syn::Error::new_spanned(
            ident,
            "Projection needs at least one field that is not `#[oxider(skip)]`",
        ));
    }

    // The impl is generic over the row rather than over one backend, so a single
    // derive serves SQLite, PostgreSQL and MySQL. The bounds say exactly what
    // that costs: every field has to decode from whatever database the row came
    // from, and the row has to accept a positional index.
    Ok(quote! {
        impl<'r, R> ::oxider_query_exec::Projection<'r, R> for #ident
        where
            R: ::sqlx::Row,
            ::core::primitive::usize: ::sqlx::ColumnIndex<R>,
            #(#decode_bounds,)*
        {
            const ARITY: ::core::primitive::usize = #arity;

            fn from_row_at(
                row: &'r R,
                offset: ::core::primitive::usize,
            ) -> ::core::result::Result<Self, ::sqlx::Error> {
                ::core::result::Result::Ok(#ident {
                    #(#reads,)*
                    #(#skipped,)*
                })
            }
        }
    })
}

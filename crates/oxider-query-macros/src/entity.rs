//! `#[derive(Entity)]`: the query metamodel for a table-backed struct.

use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::attributes::{option_inner, ColumnOptions, EntityOptions};

pub(crate) fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &input.ident;
    let options = EntityOptions::parse(&input)?;
    let table = &options.table;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "Entity requires a struct with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                ident,
                "Entity can only be derived on structs",
            ))
        }
    };

    let mut columns = Vec::new();
    for field in fields {
        let column = ColumnOptions::parse(field)?;
        if column.skip {
            continue;
        }
        let fid = field.ident.as_ref().expect("named field");
        // A field named after a Rust keyword has to be written `r#type`, but
        // `r#` is Rust syntax and not part of the column's name. Stripping it
        // is what lets a schema use `type`, `match` or `ref` as a column.
        let col_name = column
            .name
            .unwrap_or_else(|| fid.to_string().trim_start_matches("r#").to_string());
        // Nullability is a runtime flag rather than a type-level one: widening
        // every nullable column to `Option<T>` would take the string and
        // numeric operators away from exactly the columns that need them most.
        let (col_ty, nullable) = match option_inner(&field.ty) {
            Some(inner) => (inner, true),
            None => (&field.ty, false),
        };
        let constructor = match &options.schema {
            Some(schema) => quote! {
                ::oxider_query::Column::in_schema(#schema, #table, #col_name, #nullable)
            },
            None => quote! {
                ::oxider_query::Column::new(#table, #col_name, #nullable)
            },
        };
        columns.push(quote! {
            #[allow(non_upper_case_globals)]
            pub const #fid: ::oxider_query::Column<#ident, #col_ty> = #constructor;
        });
    }

    let schema_const = match &options.schema {
        Some(schema) => quote! {
            const SCHEMA: ::core::option::Option<&'static str> = ::core::option::Option::Some(#schema);
        },
        None => quote! {},
    };

    Ok(quote! {
        impl ::oxider_query::Entity for #ident {
            const TABLE: &'static str = #table;
            #schema_const
        }

        impl #ident {
            #(#columns)*
        }
    })
}

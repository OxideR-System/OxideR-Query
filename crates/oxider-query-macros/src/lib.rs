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
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Field, Fields, GenericArgument, LitStr, PathArguments,
    Type,
};

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
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
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

/// Struct-level `#[oxider(...)]` options.
struct EntityOptions {
    table: String,
    schema: Option<String>,
}

impl EntityOptions {
    fn parse(input: &DeriveInput) -> syn::Result<Self> {
        let mut table = None;
        let mut schema = None;
        for attr in &input.attrs {
            if !attr.path().is_ident("oxider") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("table") {
                    table = Some(meta.value()?.parse::<LitStr>()?.value());
                    Ok(())
                } else if meta.path.is_ident("schema") {
                    schema = Some(meta.value()?.parse::<LitStr>()?.value());
                    Ok(())
                } else {
                    Err(meta.error("unknown `oxider` option; expected `table` or `schema`"))
                }
            })?;
        }
        Ok(EntityOptions {
            table: table.unwrap_or_else(|| input.ident.to_string().to_lowercase()),
            schema,
        })
    }
}

/// Field-level `#[oxider(...)]` options.
struct ColumnOptions {
    name: Option<String>,
    skip: bool,
}

impl ColumnOptions {
    fn parse(field: &Field) -> syn::Result<Self> {
        let mut name = None;
        let mut skip = false;
        for attr in &field.attrs {
            if !attr.path().is_ident("oxider") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("column") {
                    name = Some(meta.value()?.parse::<LitStr>()?.value());
                    Ok(())
                } else if meta.path.is_ident("skip") {
                    skip = true;
                    Ok(())
                } else {
                    Err(meta.error("unknown `oxider` option; expected `column` or `skip`"))
                }
            })?;
        }
        Ok(ColumnOptions { name, skip })
    }
}

/// If `ty` is `Option<Inner>`, return `Inner`.
fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(inner) => Some(inner),
        _ => None,
    })
}

//! Derive macro generating the query metamodel for an entity struct.
//!
//! `#[derive(Entity)]` turns a plain struct into a queryable table by generating
//! a companion `<Name>Table` metamodel struct whose fields are typed [`Column`]s,
//! plus a `Name::table()` constructor and a `Table` impl.
//!
//! Generated code references the facade crate as `::oxider_query`, so users only
//! need to depend on `oxider-query`.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr, Type};

/// Derive the query metamodel for a struct.
///
/// ```ignore
/// #[derive(Entity)]
/// #[oxider(table = "users")]
/// struct User { id: i64, name: String, email: Option<String> }
/// ```
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
    let table = table_name(&input)?;
    let meta_ident = format_ident!("{}Table", ident);

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

    let mut field_defs = Vec::new();
    let mut field_inits = Vec::new();
    for field in fields {
        let fid = field.ident.as_ref().expect("named field");
        let ty = &field.ty;
        let col_name = fid.to_string();
        let nullable = is_option(ty);

        field_defs.push(quote! {
            pub #fid: ::oxider_query::Column<<#ty as ::oxider_query::HasSqlType>::Sql>
        });
        field_inits.push(quote! {
            #fid: ::oxider_query::Column::new(#table, #col_name, #nullable)
        });
    }

    Ok(quote! {
        #[derive(Clone, Copy)]
        #[doc = concat!("Generated query metamodel for [`", stringify!(#ident), "`].")]
        pub struct #meta_ident {
            #(#field_defs,)*
        }

        impl #ident {
            /// Access the generated query metamodel for this entity.
            pub fn table() -> #meta_ident {
                #meta_ident {
                    #(#field_inits,)*
                }
            }
        }

        impl ::oxider_query::Table for #meta_ident {
            fn table_name(&self) -> &'static str {
                #table
            }
        }
    })
}

/// Read the table name from `#[oxider(table = "...")]`, defaulting to the
/// lowercased struct name.
fn table_name(input: &DeriveInput) -> syn::Result<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("oxider") {
            let mut found = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("table") {
                    let lit: LitStr = meta.value()?.parse()?;
                    found = Some(lit.value());
                    Ok(())
                } else {
                    Err(meta.error("unknown `oxider` attribute; expected `table`"))
                }
            })?;
            if let Some(name) = found {
                return Ok(name);
            }
        }
    }
    Ok(input.ident.to_string().to_lowercase())
}

/// Whether a field type is `Option<..>` (making the column nullable).
fn is_option(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

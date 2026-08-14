//! Derive macro generating the query metamodel for an entity struct.
//!
//! `#[derive(Entity)]` implements `Entity` for the struct and generates one
//! associated-const `Column` per field, so queries read as `User::id`. Generated
//! code references the facade crate as `::oxider_query`, so users only depend on
//! `oxider-query`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, LitStr, PathArguments, Type,
};

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
        let fid = field.ident.as_ref().expect("named field");
        let col_name = fid.to_string();
        // Nullable columns carry the inner type of `Option<T>`; the flag records
        // nullability until type-level nullability tracking lands.
        let (col_ty, nullable) = match option_inner(&field.ty) {
            Some(inner) => (inner, true),
            None => (&field.ty, false),
        };
        columns.push(quote! {
            #[allow(non_upper_case_globals)]
            pub const #fid: ::oxider_query::Column<#ident, #col_ty> =
                ::oxider_query::Column::new(#table, #col_name, #nullable);
        });
    }

    Ok(quote! {
        impl ::oxider_query::Entity for #ident {
            const TABLE: &'static str = #table;
        }

        impl #ident {
            #(#columns)*
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

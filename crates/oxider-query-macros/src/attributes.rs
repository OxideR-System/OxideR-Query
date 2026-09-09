//! `#[oxider(...)]` attribute parsing and the type helpers both derives share.

use syn::{DeriveInput, Field, GenericArgument, LitStr, PathArguments, Type};

/// Struct-level `#[oxider(...)]` options.
pub(crate) struct EntityOptions {
    pub(crate) table: String,
    pub(crate) schema: Option<String>,
}

impl EntityOptions {
    pub(crate) fn parse(input: &DeriveInput) -> syn::Result<Self> {
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
pub(crate) struct ColumnOptions {
    pub(crate) name: Option<String>,
    pub(crate) skip: bool,
}

impl ColumnOptions {
    pub(crate) fn parse(field: &Field) -> syn::Result<Self> {
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
pub(crate) fn option_inner(ty: &Type) -> Option<&Type> {
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

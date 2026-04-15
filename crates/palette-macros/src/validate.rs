use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Expr, Ident, Path, Token, parse_macro_input};

enum Field {
    /// `field_name: expr` — expr is Result<T, E: ReasonKey>, errors collected.
    Validated { field_name: Ident, expr: Expr },
    /// `#[plain] field_name: expr` — direct assignment, no validation.
    Plain { expr: Expr },
}

enum Ctor {
    New,
    TryNew,
}

struct ValidateInput {
    ty: Path,
    ctor: Ctor,
    fields: Vec<Field>,
}

fn has_plain_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("plain"))
}

impl Parse for ValidateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let full_path: Path = input.parse()?;

        // Split the last segment as the constructor name
        let mut segments = full_path.segments.into_iter().collect::<Vec<_>>();
        let ctor_segment = segments
            .pop()
            .ok_or_else(|| input.error("expected path like Type::new"))?;

        let ctor = match ctor_segment.ident.to_string().as_str() {
            "new" => Ctor::New,
            "try_new" => Ctor::TryNew,
            other => {
                return Err(syn::Error::new_spanned(
                    &ctor_segment.ident,
                    format!("expected `new` or `try_new`, found `{other}`"),
                ));
            }
        };

        let ty = Path {
            leading_colon: full_path.leading_colon,
            segments: segments.into_iter().collect(),
        };

        // Parse { field: expr, ... }
        let content;
        syn::braced!(content in input);

        let mut fields = Vec::new();
        while !content.is_empty() {
            let attrs = content.call(Attribute::parse_outer)?;
            let name: Ident = content.parse()?;
            content.parse::<Token![:]>()?;
            let expr: Expr = content.parse()?;
            if !content.is_empty() {
                content.parse::<Token![,]>()?;
            }

            if has_plain_attr(&attrs) {
                fields.push(Field::Plain { expr });
            } else {
                fields.push(Field::Validated {
                    field_name: name,
                    expr,
                });
            }
        }

        Ok(ValidateInput { ty, ctor, fields })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ValidateInput);
    let ty = &input.ty;

    let mut bindings = Vec::new();
    let mut ctor_args = Vec::new();

    for (i, field) in input.fields.iter().enumerate() {
        match field {
            Field::Validated { field_name, expr } => {
                let var = format_ident!("__v{}", i);
                let field_str = field_name.to_string();
                bindings.push(quote! {
                        let #var = match #expr {
                            Ok(v) => Some(v),
                            Err(e) => {
                                __errors.push(::palette_core::InputError {
                                    location: ::palette_core::Location::Body,
                                    hint: #field_str.into(),
                                    reason: ::palette_core::ReasonKey::reason_key(&e),
                help: None,
                                });
                                None
                            }
                        };
                    });
                ctor_args.push(quote! { #var.unwrap() });
            }
            Field::Plain { expr } => {
                ctor_args.push(quote! { #expr });
            }
        }
    }

    let ctor_call = match input.ctor {
        Ctor::New => quote! { Ok(<#ty>::new( #(#ctor_args),* )) },
        Ctor::TryNew => quote! { <#ty>::try_new( #(#ctor_args),* ) },
    };

    let expanded = quote! {{
        let mut __errors: Vec<::palette_core::InputError> = Vec::new();
        #(#bindings)*
        if !__errors.is_empty() {
            Err(__errors)
        } else {
            #ctor_call
        }
    }};

    expanded.into()
}

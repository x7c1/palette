use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitStr, Path, Token, parse_macro_input};

enum Field {
    Validated { field_name: LitStr, expr: Expr },
    Plain { _name: Ident, expr: Expr },
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

        let content;
        syn::parenthesized!(content in input);

        let mut fields = Vec::new();
        while !content.is_empty() {
            if content.peek(LitStr) {
                let field_name: LitStr = content.parse()?;
                content.parse::<Token![=>]>()?;
                let expr: Expr = content.parse()?;
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
                fields.push(Field::Validated { field_name, expr });
            } else {
                let name: Ident = content.parse()?;
                content.parse::<Token![=]>()?;
                let expr: Expr = content.parse()?;
                if !content.is_empty() {
                    content.parse::<Token![,]>()?;
                }
                fields.push(Field::Plain { _name: name, expr });
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
                bindings.push(quote! {
                    let #var = match #expr {
                        Ok(v) => Some(v),
                        Err(e) => {
                            __errors.push(::palette_core::FieldError {
                                field: #field_name.into(),
                                reason: ::palette_core::ReasonKey::reason_key(&e),
                            });
                            None
                        }
                    };
                });
                // In the success path, unwrap is safe because we checked __errors is empty
                ctor_args.push(quote! { #var.unwrap() });
            }
            Field::Plain { expr, .. } => {
                ctor_args.push(quote! { #expr });
            }
        }
    }

    let ctor_call = match input.ctor {
        Ctor::New => quote! { Ok(<#ty>::new( #(#ctor_args),* )) },
        Ctor::TryNew => quote! { <#ty>::try_new( #(#ctor_args),* ) },
    };

    let expanded = quote! {{
        let mut __errors: Vec<::palette_core::FieldError> = Vec::new();
        #(#bindings)*
        if !__errors.is_empty() {
            Err(__errors)
        } else {
            #ctor_call
        }
    }};

    expanded.into()
}

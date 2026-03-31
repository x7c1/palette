mod validate;

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Lit, Meta, parse_macro_input};

/// Validate multiple fields and construct a domain type, collecting all errors.
///
/// See module [`validate`] for details.
#[proc_macro]
pub fn validate(input: TokenStream) -> TokenStream {
    validate::expand(input)
}

/// Derive macro that generates `namespace()`, `value()`, and `reason_key()` methods.
///
/// # Usage
///
/// ```ignore
/// #[derive(ReasonKey)]
/// pub enum InvalidWorkflowId {
///     Empty,
///     TooLong { id: String },
///     ForbiddenChar { id: String },
/// }
/// ```
///
/// Generates:
/// - `namespace()` → derived from type name: strip `Error` suffix, then snake_case
///   (e.g. `InvalidWorkflowId` → `"invalid_workflow_id"`, `BlueprintError` → `"blueprint"`)
/// - `value()` → variant name in snake_case (e.g. `TooLong` → `"too_long"`)
/// - `reason_key()` → `"{namespace}/{value}"` (e.g. `"invalid_workflow_id/too_long"`)
///
/// Use `#[reason_namespace = "..."]` on the enum to override the derived namespace.
/// Use `#[reason = "custom_name"]` on a variant to override the derived value.
#[proc_macro_derive(ReasonKey, attributes(reason_namespace, reason))]
pub fn derive_reason_key(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let namespace = extract_namespace(&input);

    let Data::Enum(data_enum) = &input.data else {
        return syn::Error::new_spanned(&input, "ReasonKey can only be derived for enums")
            .to_compile_error()
            .into();
    };

    let value_arms: Vec<_> = data_enum
        .variants
        .iter()
        .map(|variant| {
            let ident = &variant.ident;
            let value = extract_reason_override(variant)
                .unwrap_or_else(|| to_snake_case(&ident.to_string()));

            let pattern = match &variant.fields {
                Fields::Unit => quote! { Self::#ident },
                Fields::Named(_) => quote! { Self::#ident { .. } },
                Fields::Unnamed(_) => quote! { Self::#ident(..) },
            };

            quote! { #pattern => #value }
        })
        .collect();

    let expanded = quote! {
        impl ::palette_core::ReasonKey for #name {
            fn namespace(&self) -> &str {
                #namespace
            }

            fn value(&self) -> &str {
                match self {
                    #(#value_arms,)*
                }
            }
        }
    };

    expanded.into()
}

fn extract_namespace(input: &DeriveInput) -> String {
    // Check for explicit override
    for attr in &input.attrs {
        if attr.path().is_ident("reason_namespace")
            && let Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(expr_lit) = &nv.value
            && let Lit::Str(s) = &expr_lit.lit
        {
            return s.value();
        }
    }
    // Derive from type name: strip "Error" suffix, then snake_case
    let name = input.ident.to_string();
    let stripped = name.strip_suffix("Error").unwrap_or(&name);
    to_snake_case(stripped)
}

fn extract_reason_override(variant: &syn::Variant) -> Option<String> {
    for attr in &variant.attrs {
        if attr.path().is_ident("reason")
            && let Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(expr_lit) = &nv.value
            && let Lit::Str(s) = &expr_lit.lit
        {
            return Some(s.value());
        }
    }
    None
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_conversion() {
        assert_eq!(to_snake_case("Empty"), "empty");
        assert_eq!(to_snake_case("TooLong"), "too_long");
        assert_eq!(to_snake_case("ForbiddenChar"), "forbidden_char");
        assert_eq!(to_snake_case("MissingColon"), "missing_colon");
        assert_eq!(to_snake_case("MissingReviewChild"), "missing_review_child");
    }

    #[test]
    fn namespace_from_type_name() {
        use syn::parse_quote;

        // "Error" suffix stripped
        let input: DeriveInput = parse_quote! { enum BlueprintError { A } };
        assert_eq!(extract_namespace(&input), "blueprint");

        let input: DeriveInput = parse_quote! { enum TransitionError { A } };
        assert_eq!(extract_namespace(&input), "transition");

        // No "Error" suffix — full name used
        let input: DeriveInput = parse_quote! { enum InvalidWorkflowId { A } };
        assert_eq!(extract_namespace(&input), "invalid_workflow_id");

        let input: DeriveInput = parse_quote! { enum InvalidCommentBody { A } };
        assert_eq!(extract_namespace(&input), "invalid_comment_body");

        // Explicit override takes precedence
        let input: DeriveInput = parse_quote! {
            #[reason_namespace = "custom"]
            enum Foo { A }
        };
        assert_eq!(extract_namespace(&input), "custom");
    }
}

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input, Expr};

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_uppercase() {
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }

    result
}

#[proc_macro_derive(NtexError, attributes(ntex_response))]
pub fn derive_ntex_response_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut arms_status = Vec::new();
    let mut arms_error_response = Vec::new();

    if let Data::Enum(data_enum) = &input.data {
        for variant in &data_enum.variants {
            let var_ident = &variant.ident;
            let mut status = quote! { ntex::http::StatusCode::INTERNAL_SERVER_ERROR };
            let mut err_name = to_snake_case(&var_ident.to_string());
            let mut delegate = false;
            let mut has_from_attr = false;
            let mut include_fields = true;

            for attr in &variant.attrs {
                if attr.path().is_ident("ntex_response") {
                    let _ = attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("status") || meta.path.is_ident("status_code") {
                            if let Ok(value) = meta.value() {
                                if let Ok(lit_str) = value.parse::<LitStr>() {
                                    let status_str = lit_str.value().to_uppercase();
                                    let status_ident = format_ident!("{}", status_str);
                                    status = quote! { ntex::http::StatusCode::#status_ident };
                                } else if let Ok(expr) = value.parse::<Expr>() {
                                    status = quote! { #expr };
                                }
                            }
                        } else if meta.path.is_ident("name") {
                            let value: LitStr = meta.value()?.parse()?;
                            err_name = value.value();
                        } else if meta.path.is_ident("include_fields") {
                            include_fields = true;
                        } else if meta.path.is_ident("skip_fields") {
                            include_fields = false;
                        } else if meta.path.is_ident("delegate") {
                            delegate = true;
                        }
                        Ok(())
                    });
                }
            }

            match &variant.fields {
                Fields::Unnamed(fields_unnamed) => {
                    for field in &fields_unnamed.unnamed {
                        for attr in &field.attrs {
                            if attr.path().is_ident("from") {
                                has_from_attr = true;
                                break;
                            }
                        }
                        if has_from_attr {
                            break;
                        }
                    }
                }
                Fields::Named(fields_named) => {
                    for field in &fields_named.named {
                        for attr in &field.attrs {
                            if attr.path().is_ident("from") {
                                has_from_attr = true;
                                break;
                            }
                        }
                        if has_from_attr {
                            break;
                        }
                    }
                }
                _ => {}
            }

            // If #[from], disable fields by default unless explicitly overridden
            if has_from_attr {
                include_fields = variant.attrs.iter().any(|attr| {
                    if attr.path().is_ident("ntex_response") {
                        let mut found_include = false;
                        let _ = attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("include_fields") {
                                found_include = true;
                            }
                            Ok(())
                        });
                        found_include
                    } else {
                        false
                    }
                });
            }

            let is_wrapper = matches!(&variant.fields, Fields::Unnamed(f) if f.unnamed.len() == 1);

            let is_delegate = delegate && is_wrapper;

            let err_name_lit = LitStr::new(&err_name, Span::call_site());

            if is_delegate {
                arms_status.push(quote! {
                    Self::#var_ident(ref inner) => inner.status_code(),
                });
                arms_error_response.push(quote! {
                    Self::#var_ident(ref inner) => inner.error_response(req),
                });
            } else {
                let pattern_for_status = match &variant.fields {
                    Fields::Unnamed(_) => quote! { Self::#var_ident(..) },
                    Fields::Named(_) => quote! { Self::#var_ident { .. } },
                    Fields::Unit => quote! { Self::#var_ident },
                };
                arms_status.push(quote! {
                    #pattern_for_status => #status,
                });

                let (pattern, expr_fields) = if !include_fields {
                    let pattern = match &variant.fields {
                        Fields::Unit => quote! { Self::#var_ident },
                        _ => quote! { Self::#var_ident(..) },
                    };
                    (pattern, quote! { None })
                } else {
                    match &variant.fields {
                        Fields::Unit => {
                            let pattern = quote! { Self::#var_ident };
                            (pattern, quote! { None })
                        }
                        Fields::Named(fields_named) => {
                            let field_idents: Vec<_> = fields_named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                            let pattern = quote! { Self::#var_ident { #(#field_idents),* } };
                            let inserts = field_idents.iter().map(|&ident| {
                                quote! { map.insert(stringify!(#ident).to_string(), #ident.to_string()); }
                            });
                            let expr_fields = quote! {
                                let mut map = ::std::collections::HashMap::new();
                                #(#inserts)*
                                Some(map)
                            };
                            (pattern, expr_fields)
                        }
                        Fields::Unnamed(fields_unnamed) => {
                            let num = fields_unnamed.unnamed.len();
                            let field_patterns: Vec<_> = (0..num).map(|i| format_ident!("f{}", i)).collect();
                            let pattern = quote! { Self::#var_ident(#(#field_patterns),*) };
                            let inserts = (0..num).zip(&field_patterns).map(|(i, ident)| {
                                let key_lit = if num == 1 && has_from_attr {
                                    LitStr::new("cause", Span::call_site())
                                } else {
                                    LitStr::new(&i.to_string(), Span::call_site())
                                };
                                quote! { map.insert(#key_lit.to_string(), #ident.to_string()); }
                            });
                            let expr_fields = quote! {
                                let mut map = ::std::collections::HashMap::new();
                                #(#inserts)*
                                Some(map)
                            };
                            (pattern, expr_fields)
                        }
                    }
                };

                let response_arm = quote! {
                    #pattern => {
                        let code = #err_name_lit.to_string();
                        let message = self.to_string();
                        let fields = #expr_fields;
                        let result = shl_ntex::error::NtexErrorResponse {
                            code,
                            message,
                            fields,
                        };
                        ntex::web::HttpResponse::build(self.status_code()).json(&result)
                    }
                };
                arms_error_response.push(response_arm);
            }
        }
    } else {
        return syn::Error::new_spanned(name, "NtexError only works on enums").to_compile_error().into();
    }

    let expanded = quote! {
        impl ntex::web::WebResponseError for #name {
            fn status_code(&self) -> ntex::http::StatusCode {
                match self {
                    #(#arms_status)*
                }
            }
            fn error_response(&self, req: &ntex::web::HttpRequest) -> ntex::web::HttpResponse {
                match self {
                    #(#arms_error_response)*
                }
            }
        }
    };
    TokenStream::from(expanded)
}
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input};

#[proc_macro_derive(NtexError, attributes(ntex_response))]
pub fn derive_ntex_response_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let mut arms_status = Vec::new();
    let mut arms_name = Vec::new();
    let mut arms_fields = Vec::new();

    if let Data::Enum(data_enum) = &input.data {
        for variant in &data_enum.variants {
            let var_ident = &variant.ident;
            let mut status = quote! { StatusCode::INTERNAL_SERVER_ERROR };
            let mut err_name = var_ident.to_string().to_lowercase();

            for attr in &variant.attrs {
                if attr.path().is_ident("ntex_response") {
                    let _ = attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("status") {
                            let value: LitStr = meta.value()?.parse()?;
                            let status_str = value.value().to_uppercase();
                            let status_ident = format_ident!("{}", status_str);
                            status = quote! { StatusCode::#status_ident };
                        } else if meta.path.is_ident("name") {
                            let value: LitStr = meta.value()?.parse()?;
                            err_name = value.value();
                        }
                        Ok(())
                    });
                }
            }

            let err_name_lit = LitStr::new(&err_name, Span::call_site());

            let pattern = match &variant.fields {
                Fields::Unnamed(_) => quote! { Self::#var_ident(..) },
                Fields::Named(_) => quote! { Self::#var_ident { .. } },
                Fields::Unit => quote! { Self::#var_ident },
            };

            arms_status.push(quote! {
                #pattern => #status,
            });
            arms_name.push(quote! {
                #pattern => #err_name_lit,
            });

            let fields_arm = match &variant.fields {
                Fields::Named(fields_named) => {
                    let field_idents: Vec<_> = fields_named.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                    let pattern = quote! { Self::#var_ident { #(ref #field_idents),* } };
                    let inserts = field_idents.iter().map(|&ident| {
                        quote! { map.insert(stringify!(#ident).to_string(), #ident.to_string()); }
                    });
                    quote! {
                        #pattern => {
                            let mut map = ::std::collections::HashMap::new();
                            #(#inserts)*
                            Some(map)
                        },
                    }
                }
                Fields::Unnamed(fields_unnamed) => {
                    let num = fields_unnamed.unnamed.len();
                    let field_patterns: Vec<_> = (0..num).map(|i| format_ident!("_{}", i)).collect();
                    let pattern = quote! { Self::#var_ident(#(ref #field_patterns),*) };
                    let inserts = (0..num).zip(field_patterns.iter()).map(|(i, ident)| {
                        let i_lit = LitStr::new(&i.to_string(), Span::call_site());
                        quote! { map.insert(#i_lit.to_string(), #ident.to_string()); }
                    });
                    quote! {
                        #pattern => {
                            let mut map = ::std::collections::HashMap::new();
                            #(#inserts)*
                            Some(map)
                        },
                    }
                }
                Fields::Unit => {
                    let pattern = quote! { Self::#var_ident };
                    quote! {
                        #pattern => None,
                    }
                }
            };

            arms_fields.push(fields_arm);
        }
    } else {
        return syn::Error::new_spanned(name, "NtexError only works on enums").to_compile_error().into();
    }

    let expanded = quote! {
        impl WebResponseError for #name {
            fn status_code(&self) -> StatusCode {
                match self {
                    #(#arms_status)*
                }
            }
            fn error_response(&self, req: &HttpRequest) -> HttpResponse {
                let code = match self {
                    #(#arms_name)*
                };
                let fields = match self {
                    #(#arms_fields)*
                };
                let result = NtexErrorResponse {
                    code:    code.to_string(),
                    message: self.to_string(),
                    fields,
                };
                HttpResponse::build(self.status_code()).json(&result)
            }
        }
    };
    TokenStream::from(expanded)
}

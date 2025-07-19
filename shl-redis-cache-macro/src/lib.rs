use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{FnArg, ItemFn, LitStr, Pat, PatIdent, Token, parse_macro_input};
use syn::{Ident, bracketed};

enum KeyValueValue {
    Single(String),
    Array(Vec<String>),
}

struct KeyValue {
    key: String,
    value: KeyValueValue,
}

impl Parse for KeyValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;

        if input.peek(syn::token::Bracket) {
            let content;
            bracketed!(content in input);

            let array_lit = Punctuated::<LitStr, Comma>::parse_terminated(&content)?;
            let array_values = array_lit.iter().map(|lit| lit.value()).collect();

            Ok(KeyValue {
                key: key.to_string(),
                value: KeyValueValue::Array(array_values),
            })
        } else {
            let value_lit: LitStr = input.parse()?;

            Ok(KeyValue {
                key: key.to_string(),
                value: KeyValueValue::Single(value_lit.value()),
            })
        }
    }
}

struct CacheArgs {
    set_key: Option<String>,
    delete_keys: Vec<String>,
}

impl Parse for CacheArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut set_key = None;
        let mut delete_keys = Vec::new();

        let vars = Punctuated::<KeyValue, Comma>::parse_terminated(input)?;

        for var in vars {
            if var.key == "set" {
                if let KeyValueValue::Single(value) = var.value {
                    set_key = Some(value);
                }
            } else if var.key == "delete" {
                match var.value {
                    KeyValueValue::Single(value) => {
                        delete_keys.push(value);
                    }
                    KeyValueValue::Array(values) => {
                        delete_keys.extend(values);
                    }
                }
            }
        }

        Ok(CacheArgs { set_key, delete_keys })
    }
}

fn extract_placeholders(format_str: &str) -> Vec<usize> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < format_str.len() {
        if let Some(pos) = format_str[i..].find('{') {
            i += pos;
            let start = i + 1;
            if let Some(end_pos) = format_str[start..].find('}') {
                let end = start + end_pos;
                if let Ok(index) = format_str[start..end].parse::<usize>() {
                    result.push(index);
                }
                i = end + 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    result
}

#[proc_macro_attribute]
pub fn cache(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as CacheArgs);
    let input_fn = parse_macro_input!(input as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let fn_args = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_generics = &input_fn.sig.generics;
    let fn_body = &input_fn.block;
    let fn_vis = &input_fn.vis;
    let fn_asyncness = &input_fn.sig.asyncness;

    let mut arg_idents = Vec::new();
    for (i, arg) in fn_args.iter().enumerate() {
        if i == 0 {
            continue;
        }

        if let FnArg::Typed(pat_type) = arg
            && let Pat::Ident(PatIdent { ident, .. }) = &*pat_type.pat
        {
            arg_idents.push(ident);
        }
    }

    let delete_keys_expr = if !args.delete_keys.is_empty() {
        let mut delete_keys_formatted = Vec::new();

        for key_template in &args.delete_keys {
            let placeholders = extract_placeholders(key_template);

            if !placeholders.is_empty() {
                let mut formatted_key = key_template.clone();
                for idx in &placeholders {
                    formatted_key = formatted_key.replace(&format!("{{{idx}}}"), "{}");
                }

                let format_args: Vec<_> = placeholders
                    .iter()
                    .map(|&idx| if idx < arg_idents.len() { &arg_idents[idx] } else { &arg_idents[0] })
                    .collect();

                delete_keys_formatted.push(quote! {
                    format!(#formatted_key, #(#format_args),*)
                });
            } else {
                delete_keys_formatted.push(quote! {
                    String::from(#key_template)
                });
            }
        }

        quote! {
            let delete_keys = vec![#(#delete_keys_formatted),*];
            if result.is_ok() {
                let _ = self.cache_service.delete_keys(delete_keys).await;
            }
        }
    } else {
        quote! {}
    };

    let expanded = if let Some(cache_key_template) = args.set_key {
        let placeholders = extract_placeholders(&cache_key_template);

        let format_expr = if !placeholders.is_empty() {
            let mut formatted_key = cache_key_template.clone();
            for idx in &placeholders {
                formatted_key = formatted_key.replace(&format!("{{{idx}}}"), "{}");
            }

            let format_args: Vec<_> = placeholders
                .iter()
                .map(|&idx| if idx < arg_idents.len() { &arg_idents[idx] } else { &arg_idents[0] })
                .collect();

            quote! { format!(#formatted_key, #(#format_args),*) }
        } else {
            quote! { String::from(#cache_key_template) }
        };

        quote! {
            #fn_vis #fn_asyncness fn #fn_name #fn_generics (#fn_args) #fn_output {
                let cache_key = #format_expr;

                if let Some(cached_result) = self.cache_service.get(&cache_key).await {
                    return Ok(cached_result);
                }

                let result = #fn_body;

                if let Ok(data) = &result {
                    let _ = self.cache_service.set(&cache_key, data).await;
                }

                #delete_keys_expr
                result
            }
        }
    } else {
        quote! {
            #fn_vis #fn_asyncness fn #fn_name #fn_generics (#fn_args) #fn_output {
                let result = #fn_body;
                #delete_keys_expr
                result
            }
        }
    };

    TokenStream::from(expanded)
}

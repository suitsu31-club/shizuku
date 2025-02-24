use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Expr, Lit, Meta, Token};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

#[derive(Debug, Clone, Default)]
struct StaticMessageDeriveOptions {
    pub subject: Option<String>,
}

impl Parse for StaticMessageDeriveOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = StaticMessageDeriveOptions::default();
        if input.is_empty() {
            return Err(syn::Error::new(input.span(), "Expected at least one option"));
        }
        let punctuated_options = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in &punctuated_options {
            match meta {
                Meta::NameValue(name_value) => {
                    let ident = name_value.path
                        .get_ident()
                        .ok_or_else(
                            || syn::Error::new(name_value.path.span(), "Expected an identifier")
                        )?;
                    if ident.to_string().as_str() != "subject" {
                        return Err(syn::Error::new(ident.span(), "Unknown option"));
                    }
                    let Expr::Lit(lit) = &name_value.value else { 
                        return Err(syn::Error::new(name_value.value.span(), "Expected a literal"));
                    };
                    let Lit::Str(lit_str) = &lit.lit else {
                        return Err(syn::Error::new(lit.span(), "Expected a string literal"));
                    };
                    options.subject = Some(lit_str.value());
                }
                _ => return Err(syn::Error::new(meta.span(), "Unknown option")),
            }
        }
        if options.subject.is_none() {
            return Err(syn::Error::new(punctuated_options.span(), "Missing subject option"));
        }
        Ok(options)
    }
}

impl StaticMessageDeriveOptions {
    pub fn implement(self, ident: Ident) -> TokenStream {
        let subject = self.subject.unwrap();
        quote! {
            impl ame_bus::message::StaticSubjectNatsMessage for #ident {
                fn subject() -> String {
                    #subject.to_string()
                }
            }
        }
    }
}

pub fn message(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let options = syn::parse_macro_input!(attr as StaticMessageDeriveOptions);
    let item = syn::parse_macro_input!(item as syn::Item);
    let ident = match &item {
        syn::Item::Struct(item_struct) => item_struct.ident.clone(),
        syn::Item::Enum(item_enum) => item_enum.ident.clone(),
        _ => return syn::Error::new(item.span(), "Expected a struct or an enum").to_compile_error().into(),
    };
    let implementation = options.implement(ident);
    let output = quote! {
        #item
        #implementation
    };
    output.into()
}
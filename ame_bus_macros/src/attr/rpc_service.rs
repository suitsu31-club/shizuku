use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Expr, Meta, Token};

#[derive(Debug, Default)]
struct RpcOptions {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub queue_group: Option<String>,
}

impl Parse for RpcOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = RpcOptions::default();
        if input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "Expected at least one attribute",
            ));
        }
        let punctuated_options = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in &punctuated_options {
            match meta {
                // might be these attributes
                // - `name` (required)
                // - `description`
                // - `version`
                // - `queue_group`
                Meta::NameValue(name_value) => {
                    let ident = name_value.path.get_ident().ok_or(syn::Error::new_spanned(
                        &name_value.path,
                        "expected identifier",
                    ))?;
                    let ident_str = ident.to_string();
                    let Expr::Lit(value) = name_value.to_owned().value else {
                        return Err(syn::Error::new_spanned(
                            &name_value.value,
                            "expected literal",
                        ));
                    };
                    match (ident_str.as_str(), value.lit) {
                        // #[rpc_service(name = "service_name")]
                        ("name", syn::Lit::Str(name)) => {
                            result.name = Some(name.value());
                        }
                        // #[rpc_service(description = "service_description")]
                        ("description", syn::Lit::Str(description)) => {
                            result.description = Some(description.value());
                        }
                        // #[rpc_service(version = "service_version")]
                        ("version", syn::Lit::Str(version)) => {
                            result.version = Some(version.value());
                        }
                        // #[rpc_service(queue_group = "queue_group")]
                        ("queue_group", syn::Lit::Str(queue_group)) => {
                            result.queue_group = Some(queue_group.value());
                        }
                        // can't be anything else
                        (other, _) => {
                            return Err(syn::Error::new_spanned(
                                &other,
                                "expected name-value pair or flag",
                            ));
                        }
                    }
                }
                // can't be anything else
                other => {
                    return Err(syn::Error::new_spanned(
                        &other,
                        "expected name-value pair or flag",
                    ));
                }
            }
        }

        // verify required fields
        if result.name.is_none() {
            return Err(syn::Error::new(input.span(), "Expected `name` attribute"));
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
struct ParsedRpcOptions {
    pub service_name: String,
    pub service_description: Option<String>,
    pub service_version: Option<String>,
    pub queue_group: Option<String>,
}

impl Into<ParsedRpcOptions> for RpcOptions {
    fn into(self) -> ParsedRpcOptions {
        ParsedRpcOptions {
            service_name: self.name.unwrap(),
            service_description: self.description,
            service_version: self.version,
            queue_group: self.queue_group,
        }
    }
}

impl ParsedRpcOptions {
    pub fn impl_nats_service_meta_trait(self, ident: Ident) -> TokenStream {
        let Self {
            service_name,
            service_description,
            service_version,
            queue_group,
        } = self;
        let service_version = service_version.unwrap_or("0.1.0".to_string());
        let service_description = match service_description {
            Some(desc) => quote! {
                Some(#desc)
            },
            None => quote! {
                None
            },
        };
        let queue_group = match queue_group {
            Some(group) => quote! {
                Some(#group)
            },
            None => quote! {
                None
            },
        };
        quote! {
            impl ame_bus::service_rpc::NatsRpcServiceMeta for #ident {
                const SERVICE_NAME: &'static str = #service_name;
                const SERVICE_VERSION: &'static str = #service_version;
                const SERVICE_DESCRIPTION: Option<&'static str> = #service_description;
                const QUEUE_GROUP: Option<&'static str> = #queue_group;
            }
        }
    }
    pub fn to_config_tokens(self) -> TokenStream {
        let Self {
            service_name,
            service_description,
            service_version,
            queue_group,
        } = self;
        let service_version = service_version.unwrap_or("0.1.0".to_string());
        let service_description = service_description.map(|desc| {
            quote! {
                description: Some(#desc)
            }
        });
        let queue_group = queue_group.map(|group| {
            quote! {
                queue_group: Some(#group)
            }
        });
        quote! {
            async_nats::service::Config {
                name: #service_name.to_owned(),
                version: #service_version.to_owned(),
                #service_description
                #queue_group
                ..Default::default()
            }
        }
    }
    pub fn impl_nats_rpc_service_trait(self, ident: Ident) -> TokenStream {
        let config = self.to_config_tokens();
        quote! {
            #[async_trait::async_trait]
            impl ame_bus::service_rpc::NatsRpcService for #ident {
                async fn set_up_service(
                    nats: &async_nats::Client,
                ) -> anyhow::Result<async_nats::service::Service> {
                    use async_nats::service::ServiceExt;
                    let service = nats
                        .add_service(#config)
                        .await?;
                    Ok(service)
                }
            }
        }
    }
}

pub fn rpc_service_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let options = parse_macro_input!(attr as RpcOptions);
    let input = parse_macro_input!(item as syn::ItemStruct);
    let ident = input.ident.clone();
    let parsed_options: ParsedRpcOptions = options.into();
    let service_meta_impl = parsed_options.clone().impl_nats_service_meta_trait(ident.clone());
    let service_impl = parsed_options.impl_nats_rpc_service_trait(ident.clone());
    quote! {
        #input
        #service_meta_impl
        #service_impl
    }.into()
}
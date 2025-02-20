use proc_macro::{TokenStream};
use proc_macro2::Ident;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Expr, ItemStruct, Lit, Meta, Token};

#[derive(Debug, Clone, Default)]
struct JetStreamMetaOptions {
    pub stream_name: String,
    pub max_bytes: Option<i64>,
    pub max_messages: Option<i64>,
    pub no_ack: Option<bool>,
    pub description: Option<String>,
}

impl Parse for JetStreamMetaOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut stream_name = None;
        let mut max_bytes = None;
        let mut max_messages = None;
        let mut no_ack = None;
        let mut description = None;
        if input.is_empty() {
            return Err(input.error("expected jetstream options"));
        }
        let punctuated_options = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;

        for meta in & punctuated_options {
            match meta {
                // might be these attributes:
                // - `name` (required)
                // - `max_bytes`
                // - `max_messages`
                // - `description`
                // - `no_ack`
                Meta::NameValue(name_value) => {
                    let ident = name_value.path.get_ident().ok_or(syn::Error::new_spanned(
                        &name_value.path,
                        "expected identifier",
                    ))?;
                    let ident_str = ident.to_string();
                    let value = name_value.to_owned().value;
                    let Expr::Lit(value) = value else {
                        return Err(syn::Error::new_spanned(
                            value,
                            "expected literal",
                        ));
                    };
                    match (ident_str.as_str(), value.lit) {
                        // #[jetstream(name = "foo")]
                        ("name", Lit::Str(lit_str)) => {
                            stream_name = Some(lit_str.value());
                        },
                        // #[jetstream(max_bytes = 1000)]
                        ("max_bytes", Lit::Int(lit_int)) => {
                            max_bytes = Some(lit_int.base10_parse()?);
                        },
                        // #[jetstream(max_messages = 1000)]
                        ("max_messages", Lit::Int(lit_int)) => {
                            max_messages = Some(lit_int.base10_parse()?);
                        },
                        // #[jetstream(description = "foo")]
                        ("description", Lit::Str(lit_str)) => {
                            description = Some(lit_str.value());
                        },
                        // #[jetstream(no_ack)]
                        ("no_ack", Lit::Bool(lit_bool)) => {
                            no_ack = Some(lit_bool.value);
                        },

                        // unknown attribute
                        (name, _) => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!("unexpected attribute: `{}`", name),
                            ));
                        }
                    }
                }

                // might be these attributes:
                // - `no_ack`
                Meta::Path(path) => {
                    let ident = path.get_ident().ok_or(syn::Error::new_spanned(
                        &path,
                        "expected identifier",
                    ))?;
                    let ident_str = ident.to_string();
                    match ident_str.as_str() {
                        // #[jetstream(no_ack)]
                        "no_ack" => {
                            no_ack = Some(true);
                        },

                        // unknown attribute
                        other => {
                            return Err(syn::Error::new_spanned(
                                &path,
                                format!("unexpected attribute: `{}`", other),
                            ));
                        }
                    }
                },

                // can't be anything else
                other => {
                    return Err(syn::Error::new_spanned(
                        &other,
                        "expected name-value pair or flag",
                    ));
                }
            }
        }

        let Some(stream_name) = stream_name else {
            return Err(syn::Error::new_spanned(
                punctuated_options,
                "expected `name` attribute",
            ));
        };

        Ok(Self {
            stream_name,
            max_bytes,
            max_messages,
            no_ack,
            description,
        })
    }
}

impl JetStreamMetaOptions {
    fn implement(self, ident: Ident) -> proc_macro2::TokenStream {
        let Self {
            stream_name,
            max_bytes,
            max_messages,
            no_ack,
            description,
        } = self;

        let max_bytes = max_bytes.map(|v| quote! { max_bytes: #v, });
        let max_messages = max_messages.map(|v| quote! { max_messages: #v, });
        let no_ack = no_ack.map(|v| quote! { no_ack: #v, });
        let description = description.map(|v| quote! { description: Some(#v.to_owned()), });

        quote! {

            #[async_trait::async_trait]
            impl ame_bus::jetstream::NatsJetStreamMeta for #ident {
                const STREAM_NAME: &'static str = #stream_name;
                async fn get_or_create_stream(
                    &self,
                    js: &async_nats::jetstream::Context,
                ) -> anyhow::Result<async_nats::jetstream::stream::Stream> {
                    let stream = js
                        .get_or_create_stream(async_nats::jetstream::stream::Config {
                            name: Self::STREAM_NAME.to_owned(),
                            #max_bytes
                            #max_messages
                            #no_ack
                            #description
                            ..Default::default()
                        })
                        .await?;
                    Ok(stream)
                }
            }
        }.into()
    }
}

pub fn jetstream_meta(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_macro_input!(attr as JetStreamMetaOptions);
    let input = parse_macro_input!(item as ItemStruct);
    let ident = input.ident.clone();
    let implement = options.implement(ident);

    quote! {
        #input
        #implement
    }.into()
}
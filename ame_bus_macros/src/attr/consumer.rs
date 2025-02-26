use convert_case::{Case, Casing};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Expr, Lit, Meta, Token};

#[derive(Debug, Clone, Default)]
/// middleware options for jetstream consumers
/// used when parsing the `#[jetstream_consumer]` attribute
/// will be used to generate [ParsedJetStreamConsumerConfig] before generating the code.
struct JetStreamConsumerOptions {
    pub consumer_name: Option<String>,

    /// let the consumer be a push consumer.
    ///
    /// If this is set to `true`, while `is_pull` is also set to `true`,
    /// will cause a compile error.
    pub is_push: Option<bool>,

    /// let the consumer be a pull consumer.
    ///
    /// If this is set to `true`, while `is_push` is also set to `true`,
    /// will cause a compile error.
    pub is_pull: Option<bool>,

    /// let the consumer be durable.
    ///
    /// If `durable_name` is not set, will use `consumer_name`.
    ///
    /// Default is `false`.
    pub is_durable: Option<bool>,

    /// let the consumer be durable.
    ///
    /// With this option, you can set the durable name.
    /// If this is set, but `is_durable` is not set, the consumer will still be durable.
    pub durable_name: Option<String>,
    pub description: Option<String>,
    pub deliver_policy: DeliverPolicy,
    pub ack_policy: AckPolicy,
    pub ack_wait_secs: Option<i64>,
    pub filter_subject: Option<String>,
    pub headers_only: Option<bool>,
    /// pull consumer option `max_batch`
    pub pull_max_batch: Option<i64>,
    /// push consumer option `deliver_subject`
    ///
    /// With a deliver subject, the server will push messages to clients subscribed to this subject.
    ///
    /// Cannot be empty if `is_push` is set to `true`.
    pub push_deliver_subject: Option<String>,
    /// push consumer option `deliver_group`
    pub push_deliver_group: Option<String>,
}

#[derive(Debug, Clone, Default)]
enum DeliverPolicy {
    #[default]
    All,
    Last,
    New,
    LastPerSubject,
}

#[derive(Debug, Clone, Default)]
enum AckPolicy {
    All,
    #[default]
    Explicit,
    None,
}

struct ParsedJetStreamConsumerConfig {
    pub consumer_name: Option<String>,
    pub durable: DurableSetting,
    pub consumer_type: ConsumerType,
    pub deliver_policy: DeliverPolicy,
    pub ack_policy: AckPolicy,
    pub ack_wait_secs: Option<i64>,
    pub filter_subject: Option<String>,
    pub headers_only: bool,
}

enum DurableSetting {
    Ephemeral,
    Durable(String),
    DurableDefaultName,
}

enum ConsumerType {
    Push {
        deliver_subject: String,
        deliver_group: Option<String>,
    },
    Pull {
        max_batch: Option<i64>,
    },
}

impl Parse for JetStreamConsumerOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = JetStreamConsumerOptions::default();
        if input.is_empty() {
            return Ok(options);
        }
        let punctuated_options = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in &punctuated_options {
            match meta {
                // might be these attributes:
                // - `name` (required)
                // - `durable_name`
                // - `description`
                // - `deliver_policy`
                // - `ack_policy`
                // - `ack_wait_secs`
                // - `filter_subject`
                // - `headers_only`
                // - `max_batch`
                // - `deliver_subject` (only for push consumers)
                // - `deliver_group` (only for push consumers)
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
                        // #[jetstream_consumer(name = "foo")]
                        ("name", Lit::Str(lit_str)) => {
                            options.consumer_name = Some(lit_str.value());
                        }

                        // #[jetstream_consumer(durable_name = "foo")]
                        ("durable_name", Lit::Str(lit_str)) => {
                            options.durable_name = Some(lit_str.value());
                        }
                        // #[jetstream_consumer(description = "foo")]
                        ("description", Lit::Str(lit_str)) => {
                            options.description = Some(lit_str.value());
                        }

                        // #[jetstream_consumer(deliver_policy = "all")]
                        ("deliver_policy", Lit::Str(lit_str)) => {
                            options.deliver_policy = match lit_str.value().to_lowercase().as_str() {
                                "all" => DeliverPolicy::All,
                                "last" => DeliverPolicy::Last,
                                "new" => DeliverPolicy::New,
                                "last_per_subject" => DeliverPolicy::LastPerSubject,
                                _ => {
                                    return Err(syn::Error::new_spanned(
                                        lit_str,
                                        "expected one of: all, last, new, last_per_subject",
                                    ));
                                }
                            };
                        } // ("deliver_policy", Lit::Str(lit_str))

                        // #[jetstream_consumer(ack_policy = "all")]
                        ("ack_policy", Lit::Str(lit_str)) => {
                            options.ack_policy = match lit_str.value().to_lowercase().as_str() {
                                "all" => AckPolicy::All,
                                "explicit" => AckPolicy::Explicit,
                                "none" => AckPolicy::None,
                                _ => {
                                    return Err(syn::Error::new_spanned(
                                        lit_str,
                                        "expected one of: all, explicit, none",
                                    ));
                                }
                            };
                        } // ("ack_policy", Lit::Str(lit_str))

                        // #[jetstream_consumer(ack_wait_secs = 1000)]
                        ("ack_wait_secs", Lit::Int(lit_int)) => {
                            options.ack_wait_secs = Some(lit_int.base10_parse()?);
                        }

                        // #[jetstream_consumer(filter_subject = "foo")]
                        ("filter_subject", Lit::Str(lit_str)) => {
                            options.filter_subject = Some(lit_str.value());
                        }

                        // #[jetstream_consumer(headers_only)]
                        ("headers_only", Lit::Bool(lit_bool)) => {
                            options.headers_only = Some(lit_bool.value);
                        }

                        // #[jetstream_consumer(max_batch = 1000)]
                        ("max_batch", Lit::Int(lit_int)) => {
                            options.pull_max_batch = Some(lit_int.base10_parse()?);
                        }

                        // #[jetstream_consumer(deliver_subject = "foo")]
                        ("deliver_subject", Lit::Str(lit_str)) => {
                            options.push_deliver_subject = Some(lit_str.value());
                        }

                        // #[jetstream_consumer(deliver_group = "foo")]
                        ("deliver_group", Lit::Str(lit_str)) => {
                            options.push_deliver_group = Some(lit_str.value());
                        }

                        // unknown attribute
                        (name, _) => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!("unexpected attribute: `{}`", name),
                            ));
                        }
                    } // match (ident_str.as_str(), value.lit)
                } // Meta::NameValue(name_value)

                // might be these attributes:
                // - `push`
                // - `pull`
                // - `durable`
                // - `headers_only`
                Meta::Path(path) => {
                    let ident = path
                        .get_ident()
                        .ok_or(syn::Error::new_spanned(path, "expected identifier"))?;
                    let ident_str = ident.to_string();
                    match ident_str.as_str() {
                        // #[jetstream_consumer(push)]
                        "push" => {
                            options.is_push = Some(true);
                        }

                        // #[jetstream_consumer(pull)]
                        "pull" => {
                            options.is_pull = Some(true);
                        }

                        // #[jetstream_consumer(durable)]
                        "durable" => {
                            options.is_durable = Some(true);
                        }

                        // #[jetstream_consumer(headers_only)]
                        "headers_only" => {
                            options.headers_only = Some(true);
                        }

                        // unknown attribute
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "expected name-value pair or flag",
                            ));
                        }
                    } // match ident_str.as_str()
                } // Meta::Path(path)

                // can't be anything else
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "expected name-value pair or flag",
                    ));
                }
            } // match meta
        } // for meta in &punctuated_options

        // validate the options
        // - can't set both `push` and `pull`
        if options.is_push.is_some() && options.is_pull.is_some() {
            return Err(syn::Error::new_spanned(
                punctuated_options,
                "can't set both `push` and `pull`",
            ));
        }
        // - when `push` is set, `deliver_subject` must be set
        if options.is_push.is_some() && options.push_deliver_subject.is_none() {
            return Err(syn::Error::new_spanned(
                punctuated_options,
                "expected `deliver_subject` attribute",
            ));
        }

        let is_pull_consumer = match (options.is_pull, options.is_push) {
            (Some(true), Some(true)) => {
                return Err(syn::Error::new_spanned(
                    punctuated_options,
                    "can't set both `push` and `pull`",
                ));
            }
            (Some(true), None) => true,
            (None, Some(true)) => false,
            (None, None) => true,
            _ => unreachable!(),
        };

        if is_pull_consumer {
            // - no push consumer only options are set when it's a pull consumer
            if options.push_deliver_group.is_some() {
                return Err(syn::Error::new_spanned(
                    punctuated_options,
                    "`deliver_group` can't be set when `push` is not set",
                ));
            }

            if options.push_deliver_subject.is_some() {
                return Err(syn::Error::new_spanned(
                    punctuated_options,
                    "`deliver_subject` can't be set when `push` is not set",
                ));
            }
        } else {
            // - no pull consumer only options are set when it's a push consumer
            if options.pull_max_batch.is_some() {
                return Err(syn::Error::new_spanned(
                    punctuated_options,
                    "`max_batch` can't be set when `pull` is not set",
                ));
            }
        }

        Ok(options)
    }
}

impl Into<ParsedJetStreamConsumerConfig> for JetStreamConsumerOptions {
    fn into(self) -> ParsedJetStreamConsumerConfig {
        let durable = match (&self.consumer_name, self.is_durable, self.durable_name) {
            (_, Some(true), Some(durable_name)) => DurableSetting::Durable(durable_name),
            (Some(name), Some(true), None) => DurableSetting::Durable(name.to_owned()),
            (None, Some(true), None) => DurableSetting::DurableDefaultName,
            (_, None, Some(name)) => DurableSetting::Durable(name),
            (Some(name), None, None) => DurableSetting::Durable(name.to_owned()),
            (None, None, None) => DurableSetting::DurableDefaultName,
            (_, Some(false), None) => DurableSetting::Ephemeral,
            (_, Some(false), Some(_)) => {
                panic!("`durable_name` can't be set when `durable` is set to `false`")
            }
        };

        let consumer_type = match (self.is_push, self.is_pull) {
            (Some(true), Some(true)) => panic!("can't set both `push` and `pull`"),
            (Some(true), None) => ConsumerType::Push {
                deliver_subject: self.push_deliver_subject.unwrap(),
                deliver_group: self.push_deliver_group,
            },
            (None, Some(true)) => ConsumerType::Pull {
                max_batch: self.pull_max_batch,
            },
            (None, None) => ConsumerType::Pull {
                max_batch: self.pull_max_batch,
            },
            _ => unreachable!(),
        };

        ParsedJetStreamConsumerConfig {
            consumer_name: self.consumer_name,
            durable,
            consumer_type,
            deliver_policy: self.deliver_policy,
            ack_policy: self.ack_policy,
            ack_wait_secs: self.ack_wait_secs,
            filter_subject: self.filter_subject,
            headers_only: self.headers_only.unwrap_or(false),
        }
    }
}

impl ParsedJetStreamConsumerConfig {
    pub fn with_ident(mut self, ident: Ident) -> Self {
        let ident_string = ident.to_string().to_case(Case::Snake);
        if self.consumer_name.is_none() {
            self.consumer_name = Some(ident_string.clone());
        }
        if let DurableSetting::DurableDefaultName = self.durable {
            self.durable = DurableSetting::Durable(ident_string);
        }
        self
    }
    pub fn gen_config(self) -> proc_macro2::TokenStream {
        let config_ident = match &self.consumer_type {
            ConsumerType::Push { .. } => quote! {async_nats::jetstream::consumer::push::Config},
            ConsumerType::Pull { .. } => quote! {async_nats::jetstream::consumer::pull::Config},
        };

        let type_spec_options = match self.consumer_type {
            ConsumerType::Push {
                deliver_subject,
                deliver_group,
            } => {
                let deliver_group_token =
                    deliver_group.map(|group| quote! { deliver_group: Some(#group.to_owned()) });
                quote! {
                    deliver_subject: #deliver_subject.to_owned(),
                    #deliver_group_token,
                }
            }
            ConsumerType::Pull { max_batch } => match max_batch {
                Some(batch) => quote! { max_batch: #batch, },
                None => quote! {},
            },
        };
        let durable_option = match self.durable {
            DurableSetting::Ephemeral => quote! { durable_name: None, },
            DurableSetting::Durable(name) => quote! { durable_name: Some(#name.to_owned()), },
            _ => unreachable!(),
        };
        let deliver_policy = match self.deliver_policy {
            DeliverPolicy::All => {
                quote! { deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All, }
            }
            DeliverPolicy::Last => {
                quote! { deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::Last, }
            }
            DeliverPolicy::New => {
                quote! { deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::New, }
            }
            DeliverPolicy::LastPerSubject => {
                quote! { deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::LastPerSubject, }
            }
        };
        let ack_policy = match self.ack_policy {
            AckPolicy::All => {
                quote! { ack_policy: async_nats::jetstream::consumer::AckPolicy::All, }
            }
            AckPolicy::Explicit => {
                quote! { ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit, }
            }
            AckPolicy::None => {
                quote! { ack_policy: async_nats::jetstream::consumer::AckPolicy::None, }
            }
        };
        let ack_wait_secs = match self.ack_wait_secs {
            Some(secs) => quote! { ack_wait: std::time::Duration::from_secs(#secs), },
            None => quote! {},
        };
        let filter_subject = self
            .filter_subject
            .map(|subject| quote! { filter_subject: #subject.to_owned(), });
        let headers_only = if self.headers_only {
            quote! { headers_only: true, }
        } else {
            quote! {}
        };
        quote! {
            #config_ident {
                #type_spec_options
                #durable_option
                #deliver_policy
                #ack_policy
                #ack_wait_secs
                #filter_subject
                #headers_only
                ..Default::default()
            }
        }
    }
    pub fn implement(self, ident: Ident) -> proc_macro2::TokenStream {
        let consumer_config_ident = match &self.consumer_type {
            ConsumerType::Push { .. } => quote! {async_nats::jetstream::consumer::push::Config},
            ConsumerType::Pull { .. } => quote! {async_nats::jetstream::consumer::pull::Config},
        };
        let consumer_name = self.consumer_name.clone().unwrap();
        let config = self.gen_config();
        quote! {
            #[async_trait::async_trait]
            impl ame_bus::jetstream::NatsJetStreamConsumerMeta for #ident {
                type ConsumerConfig = #consumer_config_ident;
                const CONSUMER_NAME: &'static str = #consumer_name;
                async fn get_or_create_consumer(
                    stream: async_nats::jetstream::stream::Stream,
                ) -> anyhow::Result<async_nats::jetstream::consumer::Consumer<Self::ConsumerConfig>> {
                    let consumer = stream
                        .get_or_create_consumer(
                        Self::CONSUMER_NAME,
                        #config
                    )
                        .await?;
                    Ok(consumer)
                }
            }
        }
    }
}

pub fn jetstream_consumer(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = parse_macro_input!(attr as JetStreamConsumerOptions);
    let input = parse_macro_input!(item as syn::ItemStruct);
    let ident = input.ident.clone();
    let parsed_options: ParsedJetStreamConsumerConfig = options.into();
    let parsed_options = parsed_options.with_ident(ident.clone());
    let implement = parsed_options.implement(ident);
    quote! {
        #input
        #implement
    }
    .into()
}

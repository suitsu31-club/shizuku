use proc_macro::TokenStream;

mod derive_trait;
mod attr;

#[proc_macro_derive(NatsJsonMessage)]
pub fn derive_nats_json_message(input: TokenStream) -> TokenStream {
    derive_trait::nats_json_message::derive_nats_json_message(input)
}

#[proc_macro_attribute]
pub fn jetstream(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr::stream_meta::jetstream_meta(attr, item)
}

#[proc_macro_attribute]
pub fn jetstream_consumer(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr::consumer::jetstream_consumer(attr, item)
}
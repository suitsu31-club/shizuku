use proc_macro::TokenStream;

mod attr;
mod derive_trait;

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

#[proc_macro_attribute]
pub fn rpc_service(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr::rpc_service::rpc_service_impl(attr, item)
}

#[proc_macro_attribute]
pub fn rpc_route(attr: TokenStream, item: TokenStream) -> TokenStream {
    attr::rpc_route::rpc_route_impl(attr, item)
}
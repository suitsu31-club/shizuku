use proc_macro::TokenStream;

mod derive_trait;

#[proc_macro_derive(NatsJsonMessage)]
pub fn derive_nats_json_message(input: TokenStream) -> TokenStream {
    derive_trait::nats_json_message::derive_nats_json_message(input)
}
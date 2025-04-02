use proc_macro::TokenStream;

mod attr;
mod derive_trait;

#[proc_macro_derive(NatsJsonMessage)]
pub fn derive_nats_json_message(input: TokenStream) -> TokenStream {
    derive_trait::nats_json_message::derive_nats_json_message(input)
}

#[proc_macro_derive(DeriveJetMessageSend)]
pub fn derive_jetstream_message_send(input: TokenStream) -> TokenStream {
    derive_trait::jetstream_message_send::derive_jetstream_message_send(input)
}

#[proc_macro_derive(DeriveCoreMessageSend)]
pub fn derive_core_message_send(input: TokenStream) -> TokenStream {
    derive_trait::core_message_send::derive_core_message_send(input)
}
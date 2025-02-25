use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_core_message_send(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        #[async_trait::async_trait]
        impl ame_bus::service_rpc::NatsCoreMessageSendTrait for #name {}
    }.into()
}
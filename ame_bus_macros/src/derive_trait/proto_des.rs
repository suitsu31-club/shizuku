use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_proto_des(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl ame_bus::message::ByteDeserialize for #name {
            type DeError = prost::DecodeError;

            fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
                use prost::Message;
                let bytes = bytes.as_ref();
                Self::decode(bytes)
            }
        }
    }.into()
}
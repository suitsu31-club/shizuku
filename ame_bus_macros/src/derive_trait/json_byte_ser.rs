use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_json_byte_ser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl ame_bus::message::ByteSerialize for #name {
            type SerError = serde_json::Error;

            fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError> {
                let json = serde_json::to_string(self)?;
                Ok(json.into_bytes().into())
            }
        }
    }.into()
}
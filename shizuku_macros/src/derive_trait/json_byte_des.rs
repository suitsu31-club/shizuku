use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_json_byte_des(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl shizuku::ByteDeserialize for #name {
            type DeError = serde_json::Error;

            fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
                use serde::de::Error;
                let json = std::str::from_utf8(bytes.as_ref())
                    .map_err(|_| serde_json::Error::custom("Failed to parse string from bytes"))?;
                serde_json::from_str(json)
            }
        }
    }.into()
}
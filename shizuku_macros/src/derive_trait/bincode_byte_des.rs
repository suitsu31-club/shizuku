use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_bincode_byte_des(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl shizuku::ByteDeserialize for #name {
            type DeError = bincode::error::DecodeError;

            fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
                bincode::decode_from_slice(bytes.as_ref(), bincode::config::standard().with_little_endian().with_fixed_int_encoding())
            }
        }
    }.into()
}
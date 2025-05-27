use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_bincode_byte_ser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl shizuku::ByteSerialize for #name {
            type SerError = bincode::error::EncodeError;

            fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError> {
                bincode::serialize(self).map(|v| v.into_boxed_slice())
            }
        }
    }.into()
}
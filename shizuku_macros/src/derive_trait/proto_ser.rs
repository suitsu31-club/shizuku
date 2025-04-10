use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

pub fn derive_proto_ser(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    quote! {
        impl shizuku::ByteSerialize for #name {
            type SerError = prost::EncodeError;
                
            fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError> {
                use prost::Message;
                let mut buf = Vec::new();
                self.encode(&mut buf)?;
                Ok(buf.into())
            }
        }
    }.into()
}

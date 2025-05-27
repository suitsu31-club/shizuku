use proc_macro::TokenStream;

mod attr;
mod derive_trait;

#[proc_macro_derive(ProtoSer)]
pub fn derive_proto_ser(input: TokenStream) -> TokenStream {
    derive_trait::proto_ser::derive_proto_ser(input)
}

#[proc_macro_derive(ProtoDes)]
pub fn derive_proto_des(input: TokenStream) -> TokenStream {
    derive_trait::proto_des::derive_proto_des(input)
}

#[proc_macro_derive(JsonByteSer)]
pub fn derive_json_byte_ser(input: TokenStream) -> TokenStream {
    derive_trait::json_byte_ser::derive_json_byte_ser(input)
}

#[proc_macro_derive(JsonByteDes)]
pub fn derive_json_byte_des(input: TokenStream) -> TokenStream {
    derive_trait::json_byte_des::derive_json_byte_des(input)
}

#[proc_macro_derive(BincodeByteSer)]
pub fn derive_bincode_byte_ser(input: TokenStream) -> TokenStream {
    derive_trait::bincode_byte_ser::derive_bincode_byte_ser(input)
}

#[proc_macro_derive(BincodeByteDes)]
pub fn derive_bincode_byte_des(input: TokenStream) -> TokenStream {
    derive_trait::bincode_byte_des::derive_bincode_byte_des(input)
}
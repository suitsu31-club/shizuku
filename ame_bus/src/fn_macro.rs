#[cfg(feature = "protobuf")]
#[macro_export]
/// Implement [ByteSerialize] by `prost` if it already implements `prost::Message`.
macro_rules! protobuf_ser {
    ($($t:ty),*) => {
        $(
            impl ame_bus::message::ByteSerialize for $t {
                type SerError = prost::EncodeError;
                
                fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError> {
                    use prost::Message;
                    let mut buf = Vec::new();
                    self.encode(&mut buf)?;
                    Ok(buf.into())
                }
            }
        )*
    }
}

#[cfg(feature = "protobuf")]
#[macro_export]
/// Implement [ByteDeserialize] by `prost` if it already implements `prost::Message`.
macro_rules! protobuf_des {
    ($($t:ty),*) => {
        $(
            impl ame_bus::message::ByteDeserialize for $t {
                type DeError = prost::DecodeError;

                fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
                    use prost::Message;
                    let bytes = bytes.as_ref();
                    Self::decode(bytes)
                }
            }
        )*
    }
}
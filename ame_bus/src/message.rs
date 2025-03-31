use std::sync::Arc;
use async_nats::jetstream::Context;
use compact_str::CompactString;
use smallvec::SmallVec;
use crate::error::{DeserializeError, PostProcessError, SerializeError};

/// This data can be serialized to bytes.
pub trait ByteSerialize 
    where Self: Sized,
{
    /// Error type for serialization.
    type SerError: Into<SerializeError> + std::error::Error + Send + Sync + 'static;
    
    /// serialize message to bytes. Can be any format.
    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError>;
}

/// This data can be deserialized from bytes.
pub trait ByteDeserialize 
    where Self: Sized,
{
    /// Error type for deserialization.
    type DeError: Into<DeserializeError> + std::error::Error + Send + Sync + 'static;

    /// parse message from bytes. Can be any format.
    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError>;
}

#[async_trait::async_trait]
/// Message in NATS core (including service RPC).
pub trait NatsCoreMessageSendTrait: ByteSerialize + DynamicSubjectNatsMessage {

    #[doc(hidden)]
    /// Publish the message to the NATS server.
    ///
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    async fn publish(&self, nats: &async_nats::Client) -> Result<(), PostProcessError> {
        let subject = self.subject();
        let bytes = self.to_bytes()
            .map_err(|err| Into::<SerializeError>::into(err))?;
        nats.publish(subject, bytes.to_vec().into()).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
/// Message in NATS JetStream that can be published.
pub trait JetStreamMessageSendTrait: ByteSerialize + DynamicSubjectNatsMessage {
    
    #[doc(hidden)]
    /// Publish the message to the NATS server.
    /// 
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    async fn publish(&self, js_context: &Context) -> Result<(), PostProcessError> {
        js_context.publish(
            self.subject(), 
            self.to_bytes()
                .map_err(|err| Into::<SerializeError>::into(err))?
                .to_vec().into()
        ).await?;
        Ok(())
    }
}

/// NATS Message that has a subject. 
/// 
/// Can be dynamic or static. Can be NATS core message or JetStream message.
pub trait DynamicSubjectNatsMessage {
    /// The subject of the message. Can be dynamic.
    fn subject(&self) -> String;
}

/// NATS Message that has a subject.
/// 
/// Must be static. Can be NATS core message or JetStream message.
pub trait StaticSubjectNatsMessage {
    /// The subject of the message. Must be static.
    fn subject() -> String;
}

impl<T> DynamicSubjectNatsMessage for T
where
    T: StaticSubjectNatsMessage,
{
    fn subject(&self) -> String {
        T::subject()
    }
}

// auto implement with serde_json
#[cfg(feature = "json")]
impl<T> ByteSerialize for T
where
    T: serde::Serialize,
{
    type SerError = serde_json::Error;

    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError> {
        let json = serde_json::to_string(self)?;
        Ok(json.as_bytes().to_vec().into())
    }
}

// auto implement with serde_json
#[cfg(feature = "json")]
impl<T> ByteDeserialize for T
where
    T: serde::de::DeserializeOwned,
{
    type DeError = serde_json::Error;

    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
        use serde::de::Error;
        let json = std::str::from_utf8(bytes.as_ref())
            .map_err(|_| serde_json::Error::custom("Failed to parse string from bytes"))?;
        serde_json::from_str(json)
    }
}

#[cfg(feature = "protobuf")]
impl<T> ByteSerialize for T
where
    T: prost::Message,
{
    type SerError = prost::EncodeError;

    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError> {
        let mut buf = Vec::new();
        self.encode(&mut buf)?;
        Ok(buf.into())
    }
}

#[cfg(feature = "protobuf")]
impl<T> ByteDeserialize for T
where
    T: prost::Message + Default,
{
    type DeError = prost::DecodeError;

    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
        T::decode(bytes.as_ref())
    }
}
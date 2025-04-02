use std::fmt::Display;
use crate::error::{DeserializeError, PostProcessError, SerializeError};
use async_nats::jetstream::Context;
use compact_str::CompactString;
use std::future::Future;
use async_nats::Subject;
use async_nats::subject::ToSubject;
// ---------------------------------------------

/// This data can be serialized to bytes.
pub trait ByteSerialize
where
    Self: Sized + Send + Sync,
{
    /// Error type for serialization.
    type SerError: Into<SerializeError> + std::error::Error + Send + Sync + 'static;

    /// serialize message to bytes. Can be any format.
    fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError>;
}

/// This data can be deserialized from bytes.
pub trait ByteDeserialize
where
    Self: Sized + Send + Sync,
{
    /// Error type for deserialization.
    type DeError: Into<DeserializeError> + std::error::Error + Send + Sync + 'static;

    /// parse message from bytes. Can be any format.
    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError>;
}

// ---------------------------------------------

/// Message in NATS JetStream that can be published.
pub trait JetStreamMessageSendTrait: ByteSerialize + DynamicSubjectMessage {
    #[doc(hidden)]
    /// Publish the message to the NATS server.
    ///
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    fn publish(
        &self,
        js_context: &Context,
    ) -> impl Future<Output = Result<(), PostProcessError>> + Send {
        async {
            js_context
                .publish(
                    self.subject(),
                    self.to_bytes()
                        .map_err(Into::<SerializeError>::into)?
                        .to_vec()
                        .into(),
                )
                .await?;
            Ok(())
        }
    }
}

// ---------------------------------------------

/// NATS subject path.
pub struct NatsSubjectPath(pub Box<[CompactString]>);

impl ToSubject for NatsSubjectPath {
    fn to_subject(&self) -> Subject {
        let joined = self.0.join(".");
        Subject::from(joined)
    }
}

impl Display for NatsSubjectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join("."))
    }
}

impl From<NatsSubjectPath> for String {
    fn from(val: NatsSubjectPath) -> Self {
        val.0.join(".")
    }
}

/// NATS Message that has a subject.
///
/// Can be dynamic or static. Can be NATS core message or JetStream message.
pub trait DynamicSubjectMessage {
    /// The subject of the message. Can be dynamic.
    fn subject(&self) -> NatsSubjectPath;
}

/// NATS Message that has a subject.
///
/// Must be static. Can be NATS core message or JetStream message.
pub trait StaticSubjectMessage {
    /// The subject of the message. Must be static.
    fn subject() -> NatsSubjectPath;
}

impl<T> DynamicSubjectMessage for T
where
    T: StaticSubjectMessage,
{
    fn subject(&self) -> NatsSubjectPath {
        T::subject()
    }
}

// auto implement with serde_json
#[cfg(feature = "json")]
impl<T> ByteSerialize for T
where
    T: serde::Serialize + Send + Sync,
{
    type SerError = serde_json::Error;

    fn to_bytes(&self) -> Result<Box<[u8]>, Self::SerError> {
        let json = serde_json::to_string(self)?;
        Ok(json.into_bytes().into())
    }
}

// auto implement with serde_json
#[cfg(feature = "json")]
impl<T> ByteDeserialize for T
where
    T: serde::de::DeserializeOwned + Send + Sync,
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
    T: prost::Message + Send + Sync,
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
    T: prost::Message + Send + Sync,
{
    type DeError = prost::DecodeError;

    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
        T::decode(bytes.as_ref())
    }
}

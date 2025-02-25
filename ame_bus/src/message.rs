use serde::de::Error;
use std::sync::Arc;
#[cfg(feature = "jetstream")]
use async_nats::jetstream::Context;

#[async_trait::async_trait]
/// # NATS Message
///
/// Generic trait for NATS messages.
pub trait NatsMessage
where
    Self: Sized + Send,
{
    /// Error type for serialization.
    type SerError: std::error::Error + Send + Sync + 'static;

    /// Error type for deserialization.
    type DeError: std::error::Error + Send + Sync + 'static;

    /// serialize message to bytes. Can be any format.
    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError>;

    /// parse message from bytes. Can be any format.
    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError>;
}

impl<T: NatsJsonMessage> NatsMessage for T {
    type SerError = serde_json::Error;
    type DeError = serde_json::Error;

    fn to_bytes(&self) -> Result<Arc<[u8]>, Self::SerError> {
        self.to_json_bytes()
    }

    fn parse_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Self::DeError> {
        Self::from_json_bytes(bytes)
    }
}

/// # NATS JSON Message
///
/// Implement this trait will make the struct can be serialized and deserialized to JSON bytes.
///
/// Based on `serde_json` serialization and deserialization.
///
/// implement `NatsJsonMessage` will automatically implement `NatsMessage` for the type.
pub trait NatsJsonMessage
where
    Self: serde::Serialize + for<'de> serde::de::Deserialize<'de> + Sized + Send,
{
    #[allow(missing_docs)]
    fn to_json_bytes(&self) -> Result<Arc<[u8]>, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(Arc::from(json.into_bytes()))
    }
    #[allow(missing_docs)]
    fn from_json_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, serde_json::Error> {
        let json = std::str::from_utf8(bytes.as_ref())
            .map_err(|_| serde_json::Error::custom("Failed to parse string from bytes"))?;
        let rpc_request = serde_json::from_str(json)?;
        Ok(rpc_request)
    }
}

impl NatsJsonMessage for () {}
impl NatsJsonMessage for serde_json::Value {}

#[async_trait::async_trait]
/// Message in NATS core (including service RPC).
pub trait NatsCoreMessageSendTrait: NatsMessage + DynamicSubjectNatsMessage {

    #[doc(hidden)]
    /// Publish the message to the NATS server.
    ///
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    async fn publish(&self, nats: &async_nats::Client) -> anyhow::Result<()> {
        let subject = self.subject();
        let bytes = self.to_bytes()?;
        nats.publish(subject, bytes.to_vec().into()).await?;
        Ok(())
    }
}

#[cfg(feature = "jetstream")]
#[async_trait::async_trait]
/// Message in NATS JetStream that can be published.
pub trait JetStreamMessageSendTrait: NatsMessage + DynamicSubjectNatsMessage {
    
    #[doc(hidden)]
    /// Publish the message to the NATS server.
    /// 
    /// **DO NOT OVERRIDE THIS FUNCTION.**
    async fn publish(&self, js_context: &Context) -> anyhow::Result<()> {
        js_context.publish(self.subject(), self.to_bytes()?.to_vec().into()).await?;
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
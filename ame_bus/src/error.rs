use std::fmt::{Display, Formatter};

#[derive(Debug)]
/// Error when serializing message.
pub struct SerializeError(pub anyhow::Error);

impl Display for SerializeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to serialize message: \n{}", self.0)
    }
}

impl std::error::Error for SerializeError {}

// ---------------------------------------------

#[derive(Debug)]
/// Error when publishing message.
pub enum PostProcessError {
    /// Error when serializing message.
    SerializeError(SerializeError),
    /// Error when publishing message into NATS core.
    NatsMessagePushError(async_nats::PublishError),
    /// Error when publishing message into NATS JetStream.
    JetStreamMessagePushError(async_nats::jetstream::context::PublishError),
}

impl Display for PostProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PostProcessError::SerializeError(err) => write!(f, "Failed to serialize message:\n {}", err.0),
            PostProcessError::NatsMessagePushError(err) => write!(f, "Failed to publish message:\n {}", err),
            PostProcessError::JetStreamMessagePushError(err) => write!(f, "Failed to publish message:\n {}", err),
        }
    }
}

impl std::error::Error for PostProcessError {}

impl PostProcessError {
    /// Create a new [PostProcessError] from any type that can be converted to [PostProcessError].
    pub fn new<T: Into<Self>>(err: T) -> Self {
        err.into()
    }
}

// ---------------------------------------------

#[derive(Debug)]
/// Error when deserializing message.
pub struct DeserializeError(pub anyhow::Error);

impl Display for DeserializeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to deserialize message: \n{}", self.0)
    }
}

impl std::error::Error for DeserializeError {}

// ---------------------------------------------

#[derive(Debug)]
/// Error before business logic.
pub enum PreProcessError {
    /// Error when deserializing message.
    DeserializeError(DeserializeError),
}

impl Display for PreProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PreProcessError::DeserializeError(err) => write!(f, "Failed to deserialize message:\n {}", err.0),
        }
    }
}

impl std::error::Error for PreProcessError {}

impl PreProcessError {
    /// Create a new [PreProcessError] from any type that can be converted to [PreProcessError].
    pub fn new<T: Into<Self>>(err: T) -> Self {
        err.into()
    }
}

// ---------------------------------------------

mod auto_implement {
    use crate::error::{DeserializeError, PostProcessError, PreProcessError, SerializeError};

    impl From<SerializeError> for PostProcessError {
        fn from(err: SerializeError) -> Self {
            PostProcessError::SerializeError(err)
        }
    }

    impl From<async_nats::PublishError> for PostProcessError {
        fn from(err: async_nats::PublishError) -> Self {
            PostProcessError::NatsMessagePushError(err)
        }
    }
    
    impl From<async_nats::jetstream::context::PublishError> for PostProcessError {
        fn from(err: async_nats::jetstream::context::PublishError) -> Self {
            PostProcessError::JetStreamMessagePushError(err)
        }
    }

    impl From<DeserializeError> for PreProcessError {
        fn from(err: DeserializeError) -> Self {
            PreProcessError::DeserializeError(err)
        }
    }

    #[cfg(feature = "json")]
    impl From<serde_json::Error> for SerializeError {
        fn from(err: serde_json::Error) -> Self {
            SerializeError(err.into())
        }
    }
    
    #[cfg(feature = "json")]
    impl From<serde_json::Error> for DeserializeError {
        fn from(err: serde_json::Error) -> Self {
            DeserializeError(err.into())
        }
    }
}
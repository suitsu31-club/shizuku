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
/// Error after business login
pub enum PostProcessError {
    /// Error when serializing message.
    SerializeError(SerializeError),
    /// Error when publishing message into NATS core.
    NatsMessagePushError(async_nats::PublishError),
    /// When trying to reply the request, find the `reply` is `None`.
    UnexpectedNullReplySubject,
    /// Error when publishing message into NATS JetStream.
    JetStreamMessagePushError(async_nats::jetstream::context::PublishError),
}

impl Display for PostProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PostProcessError::SerializeError(err) => write!(f, "Failed to serialize message:\n {}", err.0),
            PostProcessError::NatsMessagePushError(err) => write!(f, "Failed to publish message:\n {}", err),
            PostProcessError::UnexpectedNullReplySubject => write!(f, "Unexpected null reply subject"),
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

    /// For a JetStream message, the reply subject is null.
    UnexpectedNullReplySubject,
    
    /// Unexpected subject. Can be JstStream message or NATS message.
    UnexpectedSubject(async_nats::Subject)
}

impl Display for PreProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PreProcessError::DeserializeError(err) => write!(f, "Failed to deserialize message:\n {}", err.0),
            PreProcessError::UnexpectedNullReplySubject => write!(f, "Unexpected null reply subject"),
            PreProcessError::UnexpectedSubject(subject) => write!(f, "Unexpected subject: {}", subject),
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

#[derive(Debug)]
/// All possible errors in ame-bus
pub enum Error {
    /// Error before business logic.
    PreProcessError(PreProcessError),
    
    /// Error in business logic. 
    /// 
    /// Retry time is controlled by [RetryLayer](crate::core::processor::RetryLayer).
    BusinessError(anyhow::Error),
    
    /// Error in business logic. 
    /// 
    /// This means the retry times has reached the maximum.
    BusinessRetryReached(Box<[anyhow::Error]>),
    
    /// Error in business logic. 
    /// 
    /// This means there are bugs or unexpected errors.
    /// 
    /// Won't retry.
    BusinessPanicError(anyhow::Error),
    
    /// Error after business logic.
    PostProcessError(PostProcessError),
    
    /// Error when calling RPC service.
    RpcCallRequestError(async_nats::client::RequestError),
    
    /// Custom error.
    Custom(anyhow::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::PreProcessError(err) => write!(f, "Failed to preprocess message:\n {}", err),
            Error::BusinessError(err) => write!(f, "Failed to process message:\n {}", err),
            Error::BusinessRetryReached(err) => {
                write!(f, "Failed to process message after retry:\n {:?}", err)
            },
            Error::BusinessPanicError(err) => write!(f, "Business logic panic error:\n {}", err),
            Error::PostProcessError(err) => write!(f, "Failed to postprocess message:\n {}", err),
            Error::RpcCallRequestError(err) => write!(f, "Failed to call RPC service:\n {}", err),
            Error::Custom(err) => write!(f, "Custom error:\n {}", err),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Create a new [Error] from any type that can be converted to [Error].
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
    
    impl From<PostProcessError> for super::Error {
        fn from(err: PostProcessError) -> Self {
            super::Error::PostProcessError(err)
        }
    }
    
    impl From<PreProcessError> for super::Error {
        fn from(err: PreProcessError) -> Self {
            super::Error::PreProcessError(err)
        }
    }

    impl From<serde_json::Error> for SerializeError {
        fn from(err: serde_json::Error) -> Self {
            SerializeError(err.into())
        }
    }
    
    impl From<serde_json::Error> for DeserializeError {
        fn from(err: serde_json::Error) -> Self {
            DeserializeError(err.into())
        }
    }
    
    impl From<prost::EncodeError> for SerializeError {
        fn from(err: prost::EncodeError) -> Self {
            SerializeError(err.into())
        }
    }
    
    impl From<prost::DecodeError> for DeserializeError {
        fn from(err: prost::DecodeError) -> Self {
            DeserializeError(err.into())
        }
    }
    
    impl From<bincode::error::EncodeError> for SerializeError {
        fn from(err: bincode::error::EncodeError) -> Self {
            SerializeError(err.into())
        }
    }
    
    impl From<bincode::error::DecodeError> for DeserializeError {
        fn from(err: bincode::error::DecodeError) -> Self {
            DeserializeError(err.into())
        }
    }
}
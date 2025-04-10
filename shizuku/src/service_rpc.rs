use crate::core::{ErrorTracer, FinalProcessor, Processor};
use crate::error::{Error, PostProcessError};
use async_nats::service::Request;
use async_nats::{Message, Subject};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::sync::Arc;

/// The outermost layer of a NATS service.
pub trait FinalNatsProcessor:
    FinalProcessor<Message, Result<Bytes, Error>>
{
}

/// A marker trait, mark a processor as a service endpoint.
pub trait ServiceEndpoint: Processor<Message, Result<Bytes, Error>> {
    /// The subject of the endpoint.
    const SUBJECT: &'static str;
}

#[derive(Debug, Clone)]
/// The processor used to reply the request. Will only be used in [NatsService].
struct NatsReplyProcessor {
    nats_connection: &'static async_nats::Client,
}

impl Processor<(Option<Subject>, Bytes), Result<(), Error>> for NatsReplyProcessor {
    async fn process(&self, input: (Option<Subject>, Bytes)) -> Result<(), Error> {
        let Some(reply_subject) = input.0 else {
            return Err(Error::PostProcessError(PostProcessError::UnexpectedNullReplySubject))
        };
        self.nats_connection.publish(reply_subject, input.1).await
            .map_err(|err| Error::PostProcessError(PostProcessError::NatsMessagePushError(err)))?;
        Ok(())
    }
}

/// The Service instance
pub struct NatsService<F, Et> 
    where 
        F: FinalNatsProcessor,
        Et: ErrorTracer,
{
    processor: F,
    reply_processor: NatsReplyProcessor,
    error_tracer: Et,
}

impl<F, Et> NatsService<F, Et>
    where 
        F: FinalNatsProcessor,
        Et: ErrorTracer,
{
    /// Create a new [NatsService].
    /// 
    /// parameters:
    /// 
    /// 1. `processor`: The processor function, must implement [FinalNatsProcessor] trait.
    /// 2. `stream`: The stream of requests.
    /// 3. `nats_connection`: The NATS connection, must be `&'static async_nats::Client`.
    /// 4. `error_tracer`: The error tracer, must implement [ErrorTracer] trait. If you
    ///     don't want to trace the error, use [EmptyErrorTracer](crate::core::EmptyErrorTracer)
    pub fn new(
        processor: F,
        nats_connection: &'static async_nats::Client,
        error_tracer: Et,
    ) -> Self {
        Self {
            processor,
            reply_processor: NatsReplyProcessor { nats_connection },
            error_tracer,
        }
    }
    /// Run the service.
    pub async fn run(self, mut stream: impl Stream<Item = Request> + Unpin) {
        let processor = Arc::new(self.processor);
        let reply_processor = Arc::new(self.reply_processor);
        let error_tracer = Arc::new(self.error_tracer);
        while let Some(req) = stream.next().await {
            let reply = req.message.reply.clone();
            let processed = F::process(processor.clone(), req.message).await;
            let sent = match processed {
                Ok(bytes) => reply_processor.process((reply, bytes)).await,
                Err(err) => Err(err),
            };
            Et::process(error_tracer.clone(), sent).await;
        }
    }
}

/// Routes NATS messages to appropriate service endpoints based on their subjects.
/// 
/// Example:
/// ```rust
/// # use async_nats::Message;
/// # use shizuku::{service_route, Processor, Error, FinalProcessor};
/// # use shizuku::service_rpc::FinalNatsProcessor;
/// # use bytes::Bytes;
/// # use std::sync::Arc;
/// use shizuku::service_rpc::ServiceEndpoint;
///
/// struct CreateProcessor;     // example
/// impl Processor<Message, Result<Bytes, Error>> for CreateProcessor {
///     async fn process(&self, input: Message) -> Result<Bytes, Error> {
///         unimplemented!()
///     }
/// }
/// impl ServiceEndpoint for CreateProcessor {
///     const SUBJECT: &'static str = "your_service.create";
/// }
///
/// struct ReadProcessor;        // example
/// impl Processor<Message, Result<Bytes, Error>> for ReadProcessor {
///     async fn process(&self, input: Message) -> Result<Bytes, Error> {
///         unimplemented!()
///     }
/// }
/// impl ServiceEndpoint for ReadProcessor {
///     const SUBJECT: &'static str = "your_service.read";
/// }
///
/// struct YourService {
///     create_processor: CreateProcessor,
///     read_processor: ReadProcessor,
/// }
///
/// impl FinalProcessor<Message, Result<Bytes, Error>> for YourService {
///     async fn process(state: Arc<Self>, input: Message) -> Result<Bytes, Error> {
///         service_route![input,
///             (CreateProcessor, &state.create_processor),
///             (ReadProcessor, &state.read_processor),
///         ]
///     }
/// }
/// 
/// impl FinalNatsProcessor for YourService {}
/// ```
#[macro_export]
macro_rules! service_route {
    [$input:expr, $(($processor_type:ty, $processor:expr)),* $(,)?] => {{
        match $input.subject.as_str() {
            $(
                <$processor_type>::SUBJECT => {
                    <$processor_type>::process($processor, $input).await
                },
            )*
            _ => Err($crate::error::Error::PreProcessError(
                $crate::error::PreProcessError::UnexpectedSubject($input.subject)
            ))
        }
    }};
}
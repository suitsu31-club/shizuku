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

#[derive(Debug, Clone)]
/// The processor used to reply the request. Will only be used in [NatsService].
struct NatsReplyProcessor {
    nats_connection: &'static async_nats::Client,
}

impl Processor<(Option<Subject>, Bytes), Result<(), Error>> for NatsReplyProcessor {
    fn process<'p>(&'p self, input: (Option<Subject>, Bytes)) -> impl Future<Output=Result<(), Error>> + Send + 'p {
        async move {
            let Some(reply_subject) = input.0 else {
                return Err(Error::PostProcessError(PostProcessError::UnexpectedNullReplySubject))
            };
            self.nats_connection.publish(reply_subject, input.1).await
                .map_err(|err| Error::PostProcessError(PostProcessError::NatsMessagePushError(err)))?;
            Ok(())
        }
    }
}

/// The Service instance
pub struct NatsService<F, St, Et> 
    where 
        F: FinalNatsProcessor,
        St: Stream<Item = Request> + Unpin,
        Et: ErrorTracer,
{
    processor: F,
    stream: St,
    reply_processor: NatsReplyProcessor,
    error_tracer: Et,
}

impl<F, St, Et> NatsService<F, St, Et>
    where 
        F: FinalNatsProcessor,
        St: Stream<Item = Request> + Unpin,
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
        stream: St,
        nats_connection: &'static async_nats::Client,
        error_tracer: Et,
    ) -> Self {
        Self {
            processor,
            stream,
            reply_processor: NatsReplyProcessor { nats_connection },
            error_tracer,
        }
    }
    /// Run the service.
    pub async fn run(self) {
        let processor = Arc::new(self.processor);
        let reply_processor = Arc::new(self.reply_processor);
        let error_tracer = Arc::new(self.error_tracer);
        let mut stream = self.stream;
        while let Some(req) = stream.next().await {
            let reply = req.message.reply.clone();
            let processed = F::process(processor.clone(), req.message).await;
            let sent = match processed {
                Ok(bytes) => reply_processor.process((reply, bytes)).await,
                Err(err) => Err(err),
            };
            error_tracer.process(sent).await;
        }
    }
}
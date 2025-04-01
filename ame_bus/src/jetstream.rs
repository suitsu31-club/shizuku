use crate::core::{FinalProcessor, Processor};
use crate::error::Error;
use async_nats::jetstream::Message;
use std::sync::Arc;
use futures::StreamExt;

/// The outermost layer of a NATS JetStream consumer.
///
/// It implements [Processor] trait, not [FinalProcessor] trait.
pub trait FinalJetStreamProcessor: Processor<async_nats::Message, Result<(), Error>> {}

struct JetStreamAckProcessor<F>
where
    F: FinalJetStreamProcessor + Send + Sync,
{
    sub_processor: F,
    nats_connection: &'static async_nats::Client,
}

impl<F> FinalProcessor<Message, Result<(), Error>> for JetStreamAckProcessor<F>
where
    F: FinalJetStreamProcessor + Send + Sync,
{
    fn process(state: Arc<Self>, input: Message) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            let Some(reply) = input.message.reply.clone() else {
                return Err(Error::PreProcessError(
                    crate::error::PreProcessError::UnexpectedNullReplySubject,
                ));
            };
            let message = input.message;
            let result = state.sub_processor.process(message).await;
            if let Err(err) = result {
                Err(err)
            } else {
                state
                    .nats_connection
                    .publish(reply, "".into())
                    .await
                    .map_err(|err| {
                        Error::PostProcessError(
                            crate::error::PostProcessError::NatsMessagePushError(err),
                        )
                    })?;
                Ok(())
            }
        }
    }
}

/// The NATS JetStream consumer instance
pub struct JetStreamConsumer<F, Et>
where
    F: FinalJetStreamProcessor + Send + Sync,
    Et: crate::core::ErrorTracer,
{
    processor: JetStreamAckProcessor<F>,
    error_tracer: Et,
}

impl<F, Et> JetStreamConsumer<F, Et>
where
    F: FinalJetStreamProcessor + Send + Sync,
    Et: crate::core::ErrorTracer,
{
    /// Create a new [JetStreamConsumer].
    /// 
    /// parameters:
    /// 
    /// 1. `processor`: The processor function, must implement [FinalJetStreamProcessor] trait.
    /// 2. `nats_connection`: The NATS connection, must be `&'static async_nats::Client`.
    /// 3. `error_tracer`: The error tracer, must implement [ErrorTracer] trait. If you
    ///     don't want to trace the error, use [EmptyErrorTracer](crate::core::EmptyErrorTracer)
    pub fn new(
        processor: F,
        nats_connection: &'static async_nats::Client,
        error_tracer: Et,
    ) -> Self {
        Self {
            processor: JetStreamAckProcessor {
                sub_processor: processor,
                nats_connection,
            },
            error_tracer,
        }
    }
    
    /// Run the consumer.
    pub async fn run(self, mut stream: impl futures::Stream<Item = Message> + Unpin) {
        let processor = Arc::new(self.processor);
        let error_tracer = Arc::new(self.error_tracer);
        while let Some(msg) = stream.next().await {
            let processed = JetStreamAckProcessor::<F>::process(processor.clone(), msg).await;
            Et::process(error_tracer.clone(), processed).await;
        }
    }
}
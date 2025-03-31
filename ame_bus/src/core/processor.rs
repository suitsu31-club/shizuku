use crate::error::Error;
use std::sync::Arc;

/// A closure-like trait for processing input and returning output.
pub trait Processor<I, O>: Sized {
    #[allow(missing_docs)]
    fn process<'p>(&'p self, input: I) -> impl Future<Output = O> + Send + 'p;
}

/// The outermost layer of a NATS service.
pub trait FinalNatsProcessor: Sized {
    /// receive the message from NATS and return the response.
    fn process(
        state: Arc<Self>,
        input: async_nats::Message,
    ) -> impl Future<Output = Result<async_nats::Message, Error>> + Send;
}

/// The outermost layer of a NATS JetStream consumer.
pub trait FinalJetStreamProcessor: Sized {
    /// receive the message from NATS JetStream and return the response.
    fn process(
        state: Arc<Self>,
        input: async_nats::jetstream::message::Message,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}

/// A kind of error handler, but it can only be used for tracing the error or any
/// other thing that doesn't affect the Ok result.
pub trait ErrorTracer: Processor<Result<(), Error>, ()> {}

/// A wrapper around a [Processor]. Can do something before and after the future execution.
pub trait Layer<I, O, P: Processor<I, O>> {
    /// Wrap the processor and return the output.
    fn wrap<'wrapper, 'processor>(
        &'wrapper self,
        processor: &'processor P,
        input: I,
    ) -> impl Future<Output = O> + Send + 'wrapper + 'processor
    where
        I: 'wrapper + 'processor,
        'processor: 'wrapper;
}

/// A layer that retries the processor if it returns an error.
pub struct RetryLayer;

impl RetryLayer {
    /// The maximum number of retries for business logic.
    pub const BUSINESS_MAX_RETRY: usize = 3;
}

impl<Input, Success, P> Layer<Input, Result<Success, Error>, P> for RetryLayer
where
    P: Processor<Input, Result<Success, Error>> + Send + Sync,
    Input: Clone + Send + Sync,
{
    fn wrap<'w, 'p>(
        &'w self,
        processor: &'p P,
        input: Input,
    ) -> impl Future<Output = Result<Success, Error>> + Send + 'w + 'p
    where
        Input: 'w + 'p,
        'p: 'w,
    {
        async move {
            let mut error: Vec<anyhow::Error> = Vec::new();
            for _ in 0..Self::BUSINESS_MAX_RETRY {
                match processor.process(input.clone()).await {
                    Ok(res) => return Ok(res),
                    Err(Error::BusinessError(err)) => error.push(err),
                    Err(err) => return Err(err),
                }
            }
            Err(Error::BusinessRetryReached(error.into_boxed_slice()))
        }
    }
}

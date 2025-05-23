use crate::error::Error;
use std::sync::Arc;
use kanau::layer::Layer;
use kanau::processor::{FinalProcessor, Processor};

/// A kind of error handler, but it can only be used for tracing the error or any
/// other thing that doesn't affect the Ok result.
pub trait ErrorTracer: FinalProcessor<Result<(), Error>, ()> {}

/// An empty error tracer.
pub struct EmptyErrorTracer;

impl FinalProcessor<Result<(), Error>, ()> for EmptyErrorTracer {
    async fn process(_: Arc<Self>, _: Result<(), Error>) {
    }
}

impl ErrorTracer for EmptyErrorTracer {}

/// A layer that retries the processor if it returns an error.
pub struct RetryLayer {
    /// The maximum number of retries.
    pub max_retry: usize,
}

impl RetryLayer {
    /// Create a new [RetryLayer].
    pub fn new(max_retry: usize) -> Self {
        Self { max_retry }
    }
}

impl<Input, Success, P> Layer<Input, Result<Success, Error>, P> for RetryLayer
where
    P: Processor<Input, Result<Success, Error>> + Send + Sync,
    Input: Clone + Send + Sync,
{
    fn wrap(
        &self,
        processor: &P,
        input: Input,
    ) -> impl Future<Output = Result<Success, Error>> + Send 
    {
        let max_retry = self.max_retry;
        async move {
            let mut error: Vec<anyhow::Error> = Vec::new();
            for _ in 0..max_retry {
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

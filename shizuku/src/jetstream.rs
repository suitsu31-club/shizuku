use crate::error::Error;
use async_nats::jetstream::Message;
use futures::StreamExt;
use std::sync::Arc;
use kanau::processor::{FinalProcessor, Processor};

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
    async fn process(state: Arc<Self>, input: Message) -> Result<(), Error> {
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
                    Error::PostProcessError(crate::error::PostProcessError::NatsMessagePushError(
                        err,
                    ))
                })?;
            Ok(())
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
    ///    don't want to trace the error, use [EmptyErrorTracer](crate::core::EmptyErrorTracer)
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

#[macro_export(local_inner_macros)]
/// Define the jetstream route to build [FinalJetStreamProcessor]
/// 
/// Example:
/// 
/// ```rust
/// # use shizuku::jetstream::FinalJetStreamProcessor;
/// # use shizuku::{jet_route, Error};
/// # use shizuku::processor::Processor;
/// # use async_nats::Message;
/// 
/// struct OrderPaidProcessor; // example
/// impl Processor<Message, Result<(), Error>> for OrderPaidProcessor {
///     async fn process(&self, input: Message) -> Result<(), Error> {
///         // Handle paid order message
///         unimplemented!()
///     }
/// }
/// 
/// struct OrderCancelledProcessor; // example
/// impl Processor<Message, Result<(), Error>> for OrderCancelledProcessor {
///     async fn process(&self, input: Message) -> Result<(), Error> {
///         // Handle cancelled order message
///         unimplemented!()
///     }
/// }
/// 
/// struct InvoiceProcessor; // example
/// impl Processor<Message, Result<(), Error>> for InvoiceProcessor {
///     async fn process(&self, input: Message) -> Result<(), Error> {
///         // Handle invoice message
///         unimplemented!()
///     }
/// }
/// 
/// struct OrderService {
///     order_paid_processor: OrderPaidProcessor,
///     order_cancelled_processor: OrderCancelledProcessor,
///     invoice_processor: InvoiceProcessor,
/// }
/// 
/// impl Processor<Message, Result<(), Error>> for OrderService {
///     async fn process(&self, input: Message) -> Result<(), Error> {
///         jet_route![
///             input,
///             ["invoice"] => (&self.invoice_processor),
///             ["order", "paid", "*"] => (&self.order_paid_processor),
///             ["order", "cancelled"] => (&self.order_cancelled_processor)
///         ]
///     }
/// }
/// 
/// impl FinalJetStreamProcessor for OrderService {}
macro_rules! jet_route {
    [$message_input:expr, $(
        [$($path:literal),+] => $handler:tt
    ),+] => {{
        let message_input: async_nats::Message = $message_input;
        let _depth = 0;
        let subject: Box<[_]> = message_input.subject.as_str().split('.').collect();
        let unexpected_subject_error: Result<_, $crate::error::Error> = Err($crate::error::Error::PreProcessError(
            $crate::error::PreProcessError::UnexpectedSubject(message_input.subject.clone())
        ));
        $(
            shizuku_jetstream_path_route_helper!{
                [$($path),+] => $handler @ (message_input, _depth, subject, unexpected_subject_error)
            }
        )+
        #[allow(unreachable_code)]
        return unexpected_subject_error;
    }};
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
/// handle the path
macro_rules! shizuku_jetstream_path_route_helper {
    // ["foo"] => (handler)
    // one segment path, the handler is a processor
    (
        [$one_seg_path:literal] => ($handler:expr)
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        if $one_seg_path == "*" || $one_seg_path == ">" || $subject[$depth] == $one_seg_path {
            return $handler.process($message_input).await;
        }
    };
    
    // one segment path, the handler is a nested path
    (
        [$one_seg_path:literal] => [
            $( [$($path:literal),+] => $handler:tt ),+
        ]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        core::assert_ne!(">", $one_seg_path, "Recursive wildcard \">\" must be the last segment");
        if $one_seg_path == "*" || $subject[$depth] == $one_seg_path {
            shizuku_jetstream_path_route_helper!{
                @nest_route
                [
                    $([$($path),+] => $handler),+
                ]
                @
                ($message_input, $depth, $subject, $unexpected_subject_error)
            }
        }
    };
    
    // multi segment path, the handler is a processor
    (
        [$one_seg_path:literal, $($rest_seg_path:literal),+] => ($handler:expr)
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        core::assert_ne!(">", $one_seg_path, "Recursive wildcard \">\" must be the last segment");
        if $one_seg_path == "*" || $subject[$depth] == $one_seg_path {
            shizuku_jetstream_path_route_helper!{
                @nest_route
                [
                    [$($rest_seg_path),+] => ($handler)
                ]
                @
                ($message_input, $depth, $subject, $unexpected_subject_error)
            };
        }
    };
    
    // multi segment path, the handler is a nested path
    (
        [$one_seg_path:literal, $($rest_seg_path:literal),+] => [
            $( [$($path:literal),+] => $handler:tt ),+
        ]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        core::assert_ne!(">", $one_seg_path, "Recursive wildcard \">\" must be the last segment");
        if $one_seg_path == "*" || $subject[$depth] == $one_seg_path {
            shizuku_jetstream_path_route_helper!{
                @nest_route
                [
                    [$($rest_seg_path),+] => [$([$($path),+] => $handler),+]
                ]
                @
                ($message_input, $depth, $subject, $unexpected_subject_error)
            };
        }
    };
    
    // helper
    (
        @nest_route
        [$(
            [ $($path:literal),+ ] => $handler:tt
        ),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        let _depth = $depth + 1;
        if $subject.len() <= _depth {
            return $unexpected_subject_error;
        }
        $(
            shizuku_jetstream_path_route_helper![
                [ $($path),+ ] => $handler
                @
                ($message_input, _depth, $subject, $unexpected_subject_error)
            ];
        )+
    }
}
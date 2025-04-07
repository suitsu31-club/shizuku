use crate::core::{FinalProcessor, Processor};
use crate::error::Error;
use async_nats::jetstream::Message;
use futures::StreamExt;
use std::sync::Arc;

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

#[macro_export(local_inner_macros)]
macro_rules! jet_route {
    [$message_input:expr, $(
        [$($path:tt),+] => $handler:tt
    ),+] => {{
        let message_input: async_nats::Message = $message_input;
        let _depth = 0;
        let subject: Box<[_]> = message_input.subject.as_str().split('.').collect();
        let unexpected_subject_error: Result<_, $crate::error::Error> = Err($crate::error::Error::PreProcessError(
            $crate::error::PreProcessError::UnexpectedSubject(message_input.subject.clone())
        ));
        $(
            path_route_helper!{
                [$($path),+] => $handler @ (message_input, _depth, subject, unexpected_subject_error)
            }
        )+
        #[allow(unreachable_code)]
        return unexpected_subject_error;
    }}
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
/// handle the path
macro_rules! path_route_helper {
    // end with wildcard, the handler is a processor
    (
        [*] => ($handler:expr)
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        return $handler.process($message_input).await;
    };
    
    // end with wildcard, the handler is a nested path
    (
        [*] => [$([$path:tt] => $handler:tt),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {{
        nest_route_helper!{
            [$([$path] => $handler),+]
            @
            ($message_input, $depth, $subject, $unexpected_subject_error)
        }
    }};
    
    // recursive wildcard, the handler is a processor
    (
        [>] => ($handler:expr)
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        return $handler.process($message_input).await;
    };
    
    // when use recursive wildcard, the handler must be a processor
    // so, there is no need to handle recursive wildcard
    (
        [>] => [$([$path:tt] => $handler:tt),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        compile_error!("Recursive wildcard \">\" must be the last segment");
    };
    
    // one segment path, the handler is a processor
    (
        [$one_seg_path:expr] => ($handler:expr)
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        if $subject[$depth] == $one_seg_path {
            return $handler.process($message_input).await;
        }
    };
    
    // one segment path, the handler is a nested path
    (
        [$one_seg_path:expr] => [$([$path:tt] => $handler:tt),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        if $subject[$depth] == $one_seg_path {
            nest_route_helper!{
                [$([$path] => $handler),+]
                @
                ($message_input, $depth, $subject, $unexpected_subject_error)
            }
        }
    };
    
    // multi segment path, the handler is a processor
    (
        [$one_seg_path:expr, $($rest_seg_path:tt),+] => ($handler:expr)
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        if $subject[$depth] == $one_seg_path {
            nest_route_helper!{
                [[$($rest_seg_path),+] => ($handler)]
                @
                ($message_input, $depth, $subject, $unexpected_subject_error)
            }
        }
    };
    
    // multi segment path, the handler is a nested path
    (
        [$one_seg_path:expr, $($rest_seg_path:tt),+] => [$([$path:tt] => $handler:tt),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        if $subject[$depth] == $one_seg_path {
            nest_route_helper!{
                [$($rest_seg_path),+] => [$([$path] => $handler),+]
                @
                ($message_input, $depth, $subject, $unexpected_subject_error)
            }
        }
    };
    
    // wildcard in the beginning or middle
    (
        [*, $($rest_seg_path:tt),+] => [$([$path:tt] => $handler:tt),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {{
        nest_route_helper!{
            [$($rest_seg_path),+] => [$([$path] => $handler),+]
            @
            ($message_input, $depth, $subject, $unexpected_subject_error)
        }
    }};
    
    // recursive wildcard in the beginning or middle
    // not allowed
    (
        [>, $($rest_seg_path:tt),+] => [$([$path:tt] => $handler:tt),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    ) => {
        compile_error!("Recursive wildcard \">\" must be the last segment");
    };
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
/// add the depth by 1 and handle the nested path
macro_rules! nest_route_helper {
    {
        [$(
            [$($path:tt),*] => $handler:tt
        ),+]
        @
        ($message_input:expr, $depth: expr, $subject: expr, $unexpected_subject_error: expr)
    } => {
        let _depth = $depth + 1;
        $(
            path_route_helper![
                [$($path),*] => $handler 
                @ 
                ($message_input, _depth, $subject, $unexpected_subject_error)
            ]
        )+;
        #[allow(unreachable_code)]
        return $unexpected_subject_error;
    };
}
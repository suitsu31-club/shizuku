use std::sync::Arc;
use crate::message::ByteDeserialize;

#[async_trait::async_trait]
/// # NATS JetStream Meta
///
/// Specify the JetStream using of the struct. Usually used for a consumer or message in JetStream.
///
/// Usually implemented by [jetstream](macro@crate::jetstream) attribute.
pub trait NatsJetStreamMeta: Send + Sync {
    /// A name for the Stream. Must not have spaces, tabs or period . characters
    const STREAM_NAME: &'static str;

    /// Get or create the JetStream stream.
    async fn get_or_create_stream(
        &self,
        js: &async_nats::jetstream::Context,
    ) -> anyhow::Result<async_nats::jetstream::stream::Stream> {
        let stream = js
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: Self::STREAM_NAME.to_owned(),
                max_messages: 100_000,
                ..Default::default()
            })
            .await?;
        Ok(stream)
    }
}

#[async_trait::async_trait]
/// # NATS JetStream Consumer Meta
///
/// Configure the JetStream consumer.
///
/// Must implement [NatsJetStreamMeta] trait first.
///
/// Usually implemented by [jetstream_consumer](crate::jetstream_consumer) attribute.
pub trait NatsJetStreamConsumerMeta: Send + Sync + NatsJetStreamMeta {
    /// Consumer configuration type.
    ///
    /// Usually is [jetstream::consumer::pull::Config](async_nats::jetstream::consumer::pull::Config)
    /// or [jetstream::consumer::push::Config](async_nats::jetstream::consumer::push::Config).
    type ConsumerConfig: async_nats::jetstream::consumer::IntoConsumerConfig;

    /// Consumer name.
    ///
    /// If the consumer is durable, it will also be the durable name.
    const CONSUMER_NAME: &'static str;

    /// Get or create the JetStream consumer.
    async fn get_or_create_consumer(
        stream: async_nats::jetstream::stream::Stream,
    ) -> anyhow::Result<async_nats::jetstream::consumer::Consumer<Self::ConsumerConfig>>;
}

#[async_trait::async_trait]
/// # Subscribed JetStream Event
///
/// Implement this trait to subscribe to JetStream events.
///
/// ## Example
///
/// ```rust
/// # use ame_bus::jetstream::SubscribeJetStreamEvent;
/// # use ame_bus_macros::{jetstream, jetstream_consumer, NatsJsonMessage};
/// #[jetstream(name = "user")]
/// #[jetstream_consumer(durable)]
/// struct UserSuccessfulRegisteredConsumer {
///     database_connection: (),    // use `()` for example, should be a real connection
/// }
///
/// #[derive(serde::Serialize, serde::Deserialize, NatsJsonMessage)]
/// struct UserSuccessfulRegistered {
///     user_id: String,
///     email: String,
/// }
///
/// #[async_trait::async_trait]
/// impl SubscribeJetStreamEvent for UserSuccessfulRegistered {
///     type EventConsumer = UserSuccessfulRegisteredConsumer;
///     async fn emit(consumer: std::sync::Arc<Self::EventConsumer>, event: Self) -> anyhow::Result<()> {
///         // process the event
///         Ok(())
///     }
/// }
/// ```
pub trait SubscribeJetStreamEvent: ByteDeserialize {
    /// The stateful consumer.
    ///
    /// The consumer should have everything needed to process the event.
    /// Like database connection, `async_nats::jetstream::Context`, etc.
    type EventConsumer: NatsJetStreamConsumerMeta;

    /// Emit the event.
    async fn emit(consumer: Arc<Self::EventConsumer>, event: Self) -> anyhow::Result<()>;
}

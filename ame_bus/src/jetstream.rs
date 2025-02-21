#[async_trait::async_trait]
pub trait NatsJetStreamMeta: Send + Sync {
    const STREAM_NAME: &'static str;
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
pub trait NatsJetStreamConsumerMeta: Send + Sync + NatsJetStreamMeta {
    type ConsumerConfig: async_nats::jetstream::consumer::IntoConsumerConfig;
    const CONSUMER_NAME: &'static str;
    async fn get_or_create_consumer(
        stream: async_nats::jetstream::stream::Stream,
    ) -> anyhow::Result<async_nats::jetstream::consumer::Consumer<Self::ConsumerConfig>>;
}

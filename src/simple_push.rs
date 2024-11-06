use crate::NatsJsonMessage;

#[async_trait::async_trait]
pub trait SimplestNatsMessage: NatsJsonMessage {
    async fn push_message(&self, nats_connection: &async_nats::client::Client) -> anyhow::Result<()> {
        let subject = Self::subject();
        let bytes = self.to_json_bytes()?;
        nats_connection.publish(subject, bytes.into()).await?;
        Ok(())
    }
}


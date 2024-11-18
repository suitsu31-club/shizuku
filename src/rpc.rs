use async_nats::Subject;
use crate::NatsJsonMessage;

#[async_trait::async_trait]
pub trait NatsRpcRequest: NatsJsonMessage {
    type Response: NatsJsonMessage;
    async fn send_request(&self, nats_connection: &async_nats::client::Client) -> anyhow::Result<Self::Response> {
        let subject = Self::SUBJECT;
        let bytes = self.to_json_bytes()?;
        let response = nats_connection.request(subject, bytes.into()).await?;
        let response_bytes: Vec<u8> = response.payload.into();
        let deserialized_response = Self::Response::from_json_bytes(&response_bytes)?;
        Ok(deserialized_response)
    }
    async fn reply(
        reply_to: Option<Subject>,
        message: Self::Response,
        nats_connection: &async_nats::client::Client
    ) -> anyhow::Result<()> {
        let subject = if let Some(reply_to) = reply_to {
            reply_to
        } else {
            return Ok(());
        };
        let bytes = message.to_json_bytes()?;
        nats_connection.publish(subject, bytes.into()).await?;
        Ok(())
    }
}

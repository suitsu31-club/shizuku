use crate::NatsMessage;
use anyhow::anyhow;
use async_nats::service::ServiceExt;
use tracing::error;

pub trait NatsRpcServiceMeta {
    const SERVICE_NAME: &'static str;
    const SERVICE_VERSION: Option<&'static str> = None;
    const SERVICE_DESCRIPTION: Option<&'static str> = None;
}

#[async_trait::async_trait]
pub trait NatsRpcService: Send + Sync + NatsRpcServiceMeta {
    async fn set_up_service(
        nats: &async_nats::Client,
    ) -> anyhow::Result<async_nats::service::Service> {
        nats.add_service(async_nats::service::Config {
            name: Self::SERVICE_NAME.to_owned(),
            metadata: None,
            queue_group: Self::SERVICE_VERSION.map(|v| v.to_owned()),
            description: Self::SERVICE_DESCRIPTION.map(|d| d.to_owned()),
            stats_handler: None,
            ..Default::default()
        })
        .await
        .map_err(|e| {
            error!("NATS Error: Failed to add service\n{}", e);
            anyhow!("NATS Error: Failed to add service")
        })
    }
}

#[async_trait::async_trait]
pub trait NatsRpcRequest: NatsMessage + NatsRpcRequestMeta {
    async fn process_request(
        service_state: &Self::Service,
        request: Self,
    ) -> anyhow::Result<Self::Response>;
}

pub trait NatsRpcRequestMeta {
    const ENDPOINT_NAME: &'static str;
    type Response: NatsMessage;
    type Service: NatsRpcService;
    fn subject() -> String {
        format!(
            "$SRV.{}.{}",
            Self::Service::SERVICE_NAME,
            Self::ENDPOINT_NAME
        )
    }
}

#[async_trait::async_trait]
pub trait NatsRpcRequestSendBehavior: NatsRpcRequest {
    async fn send_request(self, nats: &async_nats::Client) -> anyhow::Result<Self::Response> {
        let subject = Self::subject();
        let response = nats.request(subject, self.to_json_bytes()?.into()).await?;
        let response = Self::Response::parse_from_bytes(&response.payload)?;
        Ok(response)
    }
}

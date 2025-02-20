use crate::NatsMessage;

pub trait NatsRpcServiceMeta {
    const SERVICE_NAME: &'static str;
    const SERVICE_VERSION: Option<&'static str> = None;
    const SERVICE_DESCRIPTION: Option<&'static str> = None;
}

#[async_trait::async_trait]
pub trait NatsRpcService: Send + Sync + NatsRpcServiceMeta {
    async fn set_up_service(
        nats: &async_nats::Client,
    ) -> anyhow::Result<async_nats::service::Service>;
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
        let response = nats
            .request(subject, self.to_bytes()?.to_vec().into())
            .await?;
        let response = Self::Response::parse_from_bytes(&response.payload)?;
        Ok(response)
    }
}

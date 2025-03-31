use std::sync::Arc;
use crate::{ByteDeserialize, ByteSerialize};
use crate::message::{DynamicSubjectNatsMessage, NatsCoreMessageSendTrait, StaticSubjectNatsMessage};

#[doc(hidden)]
pub trait NatsRpcServiceMeta {
    const SERVICE_NAME: &'static str;
    const SERVICE_VERSION: &'static str;
    const SERVICE_DESCRIPTION: Option<&'static str> = None;
    const QUEUE_GROUP: Option<&'static str> = None;
}

#[doc(hidden)]
#[async_trait::async_trait]
/// # NATS RPC Service
///
/// RPC service's state. Should have everything needed to process the request of all endpoints.
pub trait NatsRpcService: Send + Sync {
    /// Set up the [async_nats::service::Service].
    async fn set_up_service(
        nats: &async_nats::Client,
    ) -> Result<async_nats::service::Service, async_nats::Error>;
}

#[async_trait::async_trait]
/// # NATS RPC Endpoint
///
/// Implement this trait to process the request of the endpoint.
pub trait NatsRpcRequest: ByteDeserialize + NatsRpcRequestMeta {
    /// Response type of the endpoint.
    type Response: ByteDeserialize;

    /// Business logic of the endpoint.
    async fn process_request(
        service_state: &Self::Service,
        request: Self,
    ) -> anyhow::Result<Self::Response>;
}

/// # NATS RPC Endpoint
///
/// Meta trait for the endpoint.
///
/// If you don't have a good reason, you should always use macro to implement this trait.
pub trait NatsRpcRequestMeta {
    /// Name of the endpoint.
    const ENDPOINT_NAME: &'static str;

    /// Service which the endpoint belongs to.
    type Service: NatsRpcService;
}

#[async_trait::async_trait]
/// # NATS RPC Call
/// 
/// Call the RPC endpoint.
pub trait NatsRpcCallTrait: NatsRpcRequest + StaticSubjectNatsMessage + ByteSerialize {
    /// Call the RPC endpoint.
    /// 
    /// If the response is `()` or other void type, use `call_void` instead.
    async fn call(self, nats: &async_nats::Client) -> anyhow::Result<Self::Response> {
        let subject = self.subject();
        let req_bytes = self.to_bytes()?;
        let res = nats.request(subject, req_bytes.to_vec().into()).await?;
        let res = Self::Response::parse_from_bytes(res.payload)?;
        Ok(res)
    }
    
    /// Call the RPC endpoint without response.
    async fn call_void(self, nats: &async_nats::Client) -> anyhow::Result<()> {
        let subject = self.subject();
        let req_bytes = self.to_bytes()?;
        nats.publish(subject, req_bytes.to_vec().into()).await?;
        Ok(())
    }
}

impl<T> NatsRpcCallTrait for T
where
    T: NatsRpcRequest + StaticSubjectNatsMessage + ByteSerialize,
{
}

impl<T> StaticSubjectNatsMessage for T
where
    T: NatsRpcRequestMeta,
    T::Service: NatsRpcServiceMeta,
{
    fn subject() -> String {
        format!("{}.{}", T::Service::SERVICE_NAME, T::ENDPOINT_NAME)
    }
}

impl<T> NatsCoreMessageSendTrait for T
where
    T: NatsRpcRequestMeta + ByteSerialize,
    T::Service: NatsRpcServiceMeta,
{
}

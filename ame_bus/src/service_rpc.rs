use crate::message::{NatsCoreMessageSendTrait, StaticSubjectNatsMessage};
use crate::NatsMessage;

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
    ) -> anyhow::Result<async_nats::service::Service>;
}

#[async_trait::async_trait]
/// # NATS RPC Endpoint
///
/// Implement this trait to process the request of the endpoint.
pub trait NatsRpcRequest: NatsMessage + NatsRpcRequestMeta {
    /// Response type of the endpoint.
    type Response: NatsMessage;

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

impl<T> StaticSubjectNatsMessage for T
where
    T: NatsRpcRequestMeta + NatsMessage,
    T::Service: NatsRpcServiceMeta,
{
    fn subject() -> String {
        format!("service.{}.{}", T::Service::SERVICE_NAME, T::ENDPOINT_NAME)
    }
}

impl<T> NatsCoreMessageSendTrait for T
where
    T: NatsRpcRequestMeta + NatsMessage,
    T::Service: NatsRpcServiceMeta,
{
}

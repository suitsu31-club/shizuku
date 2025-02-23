use crate as ame_bus;
use ame_bus_macros::*;
use tokio::sync::OnceCell;
use serde::{Deserialize, Serialize};
use ame_bus::service_rpc::NatsRpcRequest;
use crate::service_rpc::NatsRpcRequestMeta;

static NATS_CONNECTION: OnceCell<async_nats::Client> = OnceCell::const_new();

#[rpc_service(
    name = "user.info",
    version = "0.1.0"
)]
pub struct UserInfoService {
    // fields, like database connection
}

#[derive(Debug, Clone, Serialize, Deserialize, NatsJsonMessage)]
struct UserAvatarReq {
    user_id: String,
}

impl NatsRpcRequestMeta for UserAvatarReq {
    const ENDPOINT_NAME: &'static str = "avatar";
    type Service = UserInfoService;
}

#[async_trait::async_trait]
impl NatsRpcRequest for UserAvatarReq {
    type Response = ();

    async fn process_request(_: &Self::Service, _: Self) -> anyhow::Result<Self::Response> {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, NatsJsonMessage)]
struct UserMetaReq {
    user_id: String,
}

impl NatsRpcRequestMeta for UserMetaReq {
    const ENDPOINT_NAME: &'static str = "meta";
    type Service = UserInfoService;
}

#[async_trait::async_trait]
impl NatsRpcRequest for UserMetaReq {
    type Response = ();
    async fn process_request(_: &Self::Service, _: Self) -> anyhow::Result<Self::Response> {
        Ok(())
    }
}

#[rpc_route(
    service="UserInfoService",
    nats_connection="NATS_CONNECTION.get().unwrap()"
)]
enum UserInfoRoute {
    #[rpc_endpoint(request="UserAvatarReq")]
    UserAvatar,
    #[rpc_endpoint(request="UserMetaReq")]
    UserMeta,
}
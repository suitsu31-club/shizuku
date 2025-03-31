#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

/// [JetStream](https://docs.nats.io/nats-concepts/jetstream) support.
pub mod jetstream;

/// [Key/Value Store](https://docs.nats.io/nats-concepts/jetstream/key-value-store) support.
pub mod kv;

/// Define the message struct using inside `ame-bus`.
pub mod message;

/// Tokio concurrency utilities.
pub mod pool;

/// Service RPC support. Using just NATS core features.
pub mod service_rpc;

/// Internal error types.
pub mod error;

#[cfg(test)]
mod tests;

pub use message::*;

/// # Specify the JetStream using of the struct.
/// Usually used for a consumer or message in JetStream.
///
/// ## Example:
///
/// ```rust
/// # use ame_bus_macros::jet;
/// #[jet(
///     name = "user",
///     description = "User successful registered event",
/// )]
/// pub struct UserSuccessfulRegistered {
///     pub user_id: String,
///     pub email: String,
/// }
/// ```
///
/// ## Supported attributes
///
/// - `name="foo"` (required): Stream name. Must not have spaces, tabs or period . characters.
/// - `description="foo"`: Stream description.
/// - `max_messages=100_000`: Maximum number of messages to keep in the stream.
/// - `max_bytes=1024`: Maximum bytes of messages
/// - `no_ack`: Disables acknowledging messages that are received by the Stream.
///
/// Attention that these options only work if the stream is not created yet and the stream
/// is created with these options.
pub use ame_bus_macros::jet;

/// # Configure the JetStream consumer.
///
/// Must implement [NatsJetStreamMeta](crate::jetstream::NatsJetStreamMeta) trait first.
///
/// ## Example
///
/// ```rust
/// # use ame_bus_macros::{jet, jet_consumer};
/// #[jet(
///      name = "user",
///      description = "User successful registered event consumer",
/// )]
/// #[jet_consumer(
///    name = "user-successful-registered-consumer",
///    durable,
///    filter_subject = "user.registered",
/// )]
/// pub struct UserSuccessfulRegisteredConsumer {
///     database_connection: (),    // use `()` for example, should be a real connection
/// }
/// ```
///
/// ## Supported attributes
///
/// - `name="foo"`: Consumer name. If the consumer is durable, it will also be the durable name. Default to the struct name in snake case.
/// - `push`: Use push-based consumer. Cannot be used with `pull`.
/// - `pull`: Use pull-based consumer. Cannot be used with `push`. Default if neither `push` nor `pull` is specified.
/// - `durable`: Create a durable consumer. Default is false.
/// - `durable_name="foo"`: Durable name. If not specified, the consumer name is used.
/// - `deliver_policy="all"|"last"|"new"|"last_per_subject"`: Delivery policy. Default is `all`.
/// - `ack_policy="explicit"|"all"|"none"`: Acknowledgement policy. Default is `explicit`.
/// - `ack_wait_secs=10`: Acknowledgement wait time in seconds.
/// - `filter_subject="foo.bar"`: Filter messages by subject. Allow wildcards.
/// - `headers_only`: Let payload be empty and only headers are delivered.
///
/// only with pull consumer:
/// - `max_batch=10`: Maximum number of messages to fetch in a batch.
///
/// only with push consumer:
/// - `deliver_subject="foo.bar"`: Subject to deliver messages to.
/// - `deliver_group="foo"`: Consumer group to deliver messages to.
///
/// ## Example:
///
/// ```rust
/// # use ame_bus_macros::*;
/// #[jet(name = "mail", description = "Mail service")]
/// #[jet_consumer(pull, durable, filter_subject="mail.send")]
/// struct EmailSendEventConsumer {
///     smtp_connection: (),
///     email_template: String,
/// }
/// ```
pub use ame_bus_macros::jet_consumer;

/// Implement [NatsJsonMessage](crate::message::NatsJsonMessage) trait if it has already
/// implemented `Serialize` and `Deserialize` traits.
///
/// example:
/// ```rust
/// # use serde::{Deserialize, Serialize};
/// # use ame_bus_macros::NatsJsonMessage;
/// #[derive(Serialize, Deserialize, NatsJsonMessage)]
/// pub struct User {
///    pub id: String,
///    pub name: String,
/// }
/// ```
pub use ame_bus_macros::NatsJsonMessage;

/// # RPC Service
/// 
/// Mark a struct as an RPC service.
/// 
/// ## Example
/// ```rust
/// # use ame_bus_macros::rpc_service;
/// #[rpc_service(
///     name = "user_info",
///     version = "0.1.0",
/// )]
/// pub struct UserInfoService {
///     // fields, like database connection
/// }
/// ```
/// 
/// ## Supported attributes
/// 
/// - `name="foo"` (required): Service name. Can be a NATS path.
/// - `description="foo"`: Service description.
/// - `version="0.1.0"`: Service version. Default is `0.1.0`.
/// - `queue_group="foo"`: Queue group name.
/// 
/// Usually, you need to set the `queue_group` to make the service scaled properly.
pub use ame_bus_macros::rpc_service;

/// # RPC Route Register
///
/// Register the route, and implement the [PooledApp](pool::PooledApp) trait.
///
/// ## Usage
///
/// Use an enum as route table, mark the enum with `#[rpc_route()]` attribute.
///
/// `#[rpc_route()]` must have these args:
///
/// - `service`: The service struct name.
/// - `nats_connection`: The NATS connection, should be `&async_nats::Client`.
///
/// *To avoid lifetime issue, use `&'static async_nats::Client` with `OnceCell<Client>` is suggested.*
///
/// To register the requests, each variant in enum must have `#[rpc_endpoint()]` attribute.
///
/// `#[rpc_endpoint(request = "RequestName")]` must have these args:
///
/// - `request`: the request, must implement [NatsRpcRequest](crate::service_rpc::NatsRpcRequest) trait.
///
/// ## Example
///
/// ```rust
/// # use ame_bus_macros::*;
/// # use ame_bus::service_rpc::NatsRpcRequestMeta;
/// use tokio::sync::OnceCell;
/// use serde::{Deserialize, Serialize};
/// use ame_bus::service_rpc::NatsRpcRequest;
///
/// // don't forget to set up the NATS connection
/// static NATS_CONNECTION: OnceCell<async_nats::Client> = OnceCell::const_new();
/// 
/// #[rpc_service(
///     name = "user_info",
///     version = "0.1.0",
/// )]
/// pub struct UserInfoService {
///     // fields, like database connection
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, NatsJsonMessage)]
/// struct UserAvatarReq {
///     user_id: String,
/// }
/// 
/// # impl NatsRpcRequestMeta for UserAvatarReq {
/// #     const ENDPOINT_NAME: &'static str = "avatar";
/// #     type Service = UserInfoService;
/// # }
///
/// #[async_trait::async_trait]
/// impl NatsRpcRequest for UserAvatarReq {
///     type Response = ();
///
///     async fn process_request(service_state: &Self::Service, request: Self) -> anyhow::Result<Self::Response> {
///         Ok(())
///     }
/// }
///
/// #[derive(Debug, Clone, Serialize, Deserialize, NatsJsonMessage)]
/// struct UserMetaReq {
///     user_id: String,
/// }
/// 
/// # impl NatsRpcRequestMeta for UserMetaReq {
/// #     const ENDPOINT_NAME: &'static str = "meta";
/// #     type Service = UserInfoService;
/// # }
/// 
/// #[async_trait::async_trait]
/// impl NatsRpcRequest for UserMetaReq {
///     type Response = ();
///     async fn process_request(service_state: &Self::Service, request: Self) -> anyhow::Result<Self::Response> {
///         Ok(())
///     }
/// }
///
/// #[rpc_route(service="UserInfoService", nats_connection="NATS_CONNECTION.get().unwrap()")]
/// enum UserInfoRoute {
///     #[rpc_endpoint(request="UserAvatarReq")]
///     UserAvatar,
///     #[rpc_endpoint(request="UserMetaReq")]
///     UserMeta,
/// }
/// ```
pub use ame_bus_macros::rpc_route;

/// Implement [NatsCoreMessageSendTrait](crate::message::NatsCoreMessageSendTrait) for the struct.
/// 
/// Must implement [NatsMessage](crate::message::NatsMessage) and 
/// [DynamicSubjectNatsMessage](crate::message::DynamicSubjectNatsMessage) first.
pub use ame_bus_macros::DeriveCoreMessageSend;

/// Implement [JetStreamMessageSendTrait](crate::message::JetStreamMessageSendTrait) for the struct.
/// 
/// Must implement [NatsMessage](crate::message::NatsMessage) and 
/// [DynamicSubjectNatsMessage](crate::message::DynamicSubjectNatsMessage) first.
pub use ame_bus_macros::DeriveJetMessageSend;

/// Implement [StaticSubjectNatsMessage](crate::message::StaticSubjectNatsMessage) for the struct.
/// 
/// The subject must be static. Implement [StaticSubjectNatsMessage](crate::message::StaticSubjectNatsMessage)
/// will also implement [DynamicSubjectNatsMessage](crate::message::DynamicSubjectNatsMessage) for the struct.
/// 
/// usage: 
/// 
/// ```rust
/// # use ame_bus_macros::nats_message;
/// #[nats_message(subject = "user.registered")]
/// pub struct UserRegistered {}
/// ```
pub use ame_bus_macros::nats_message;

pub use tracing;
pub use futures;
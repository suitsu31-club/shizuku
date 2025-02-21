#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![allow(rustdoc::missing_crate_level_docs)]

#[cfg(any(feature = "jetstream", doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "jetstream")))]
/// [JetStream](https://docs.nats.io/nats-concepts/jetstream) support.
pub mod jetstream;

#[cfg(any(feature = "jetstream", doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "jetstream")))]
/// [Key/Value Store](https://docs.nats.io/nats-concepts/jetstream/key-value-store) support.
pub mod kv;

/// Define the message struct using inside `ame-bus`.
pub mod message;

/// Tokio concurrency utilities.
pub mod pool;

#[cfg(any(feature = "service", doc))]
#[cfg_attr(docsrs, doc(cfg(feature = "service")))]
/// Service RPC support. Using just NATS core features.
pub mod service_rpc;
mod tests;

pub use message::{NatsJsonMessage, NatsMessage};

/// # Specify the JetStream using of the struct.
/// Usually used for a consumer or message in JetStream.
///
/// ## Example:
///
/// ```rust
/// # use ame_bus_macros::jetstream;
///
/// #[jetstream(
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
pub use ame_bus_macros::jetstream;

/// # Configure the JetStream consumer.
///
/// Must implement [NatsJetStreamMeta](crate::jetstream::NatsJetStreamMeta) trait first.
///
/// ## Example
///
/// ```rust
/// # use ame_bus_macros::{jetstream, jetstream_consumer};
///
/// #[jetstream(
///      name = "user",
///      description = "User successful registered event consumer",
/// )]
/// #[jetstream_consumer(
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
/// - `name="foo"` (required): Consumer name. If the consumer is durable, it will also be the durable name.
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
pub use ame_bus_macros::jetstream_consumer;

/// Implement `NatsJsonMessage` trait if it has already implemented `Serialize` and `Deserialize` traits.
///
/// example:
/// ```rust
/// # use serde::{Deserialize, Serialize};
/// # use ame_bus_macros::NatsJsonMessage;
///
/// #[derive(Serialize, Deserialize, NatsJsonMessage)]
/// pub struct User {
///    pub id: String,
///    pub name: String,
/// }
/// ```
pub use ame_bus_macros::NatsJsonMessage;
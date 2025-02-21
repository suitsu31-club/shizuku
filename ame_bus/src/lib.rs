#[cfg(any(feature = "jetstream", doc))]
pub mod jetstream;
#[cfg(any(feature = "jetstream", doc))]
pub mod kv;
pub mod message;
pub mod pool;
#[cfg(any(feature = "service", doc))]
pub mod service_rpc;
mod tests;

pub use message::{NatsJsonMessage, NatsMessage};

/// Specify the JetStream using of the struct.
/// Usually used for a consumer or message in JetStream.
///
/// example:
///
/// ```rust
/// # use ame_bus_macros::jetstream;
///
/// #[jetstream(
///     name = "user.registered.successful",
///     description = "User successful registered event",
/// )]
/// pub struct UserSuccessfulRegistered {
///     pub user_id: String,
///     pub email: String,
/// }
/// ```
pub use ame_bus_macros::jetstream;

/// Configure the JetStream consumer.
///
/// Must implement [NatsJetStreamMeta](crate::jetstream::NatsJetStreamMeta) trait first.
///
/// example:
/// ```rust
/// # use ame_bus_macros::{jetstream, jetstream_consumer};
///
/// #[jetstream(
///      name = "user.registered.successful",
///      description = "User successful registered event consumer",
/// )]
/// #[jetstream_consumer(
///    name = "user-successful-registered-consumer",
///    durable
/// )]
/// pub struct UserSuccessfulRegisteredConsumer {
///     database_connection: (),    // use `()` for example, should be a real connection
/// }
/// ```
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
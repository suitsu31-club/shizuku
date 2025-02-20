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

/// This macro is used to specify the JetStream using of the struct.
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

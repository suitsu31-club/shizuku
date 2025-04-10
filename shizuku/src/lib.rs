#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![doc = include_str!("../README.md")]

/// Core of Ame Bus.
/// 
/// This part defines the general traits and utilities.
pub mod core;

/// [JetStream](https://docs.nats.io/nats-concepts/jetstream) support.
pub mod jetstream;

/// [Key/Value Store](https://docs.nats.io/nats-concepts/jetstream/key-value-store) support.
pub mod kv;

/// Service RPC support. Using just NATS core features.
pub mod service_rpc;

#[cfg(test)]
mod tests;
mod fn_macro;

pub use core::message::*;
pub use core::error;
pub use core::error::Error;
pub use core::processor::*;

pub use tracing;
pub use futures;

#[cfg(feature = "json")]
/// Implement [ByteDeserialize] by `serde_json` if it already implements `serde::Deserialize`.
pub use ame_bus_macros::JsonByteDes;

#[cfg(feature = "json")]
/// Implement [ByteSerialize] by `serde_json` if it already implements `serde::Serialize`.
pub use ame_bus_macros::JsonByteSer;

#[cfg(feature = "protobuf")]
/// Implement [ByteDeserialize] by `prost` if it already implements `prost::Message`.
pub use ame_bus_macros::ProtoDes;

#[cfg(feature = "protobuf")]
/// Implement [ByteSerialize] by `prost` if it already implements `prost::Message`.
pub use ame_bus_macros::ProtoSer;
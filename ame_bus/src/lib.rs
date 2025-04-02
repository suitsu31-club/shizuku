#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
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

pub use core::message::*;
pub use core::error;

pub use tracing;
pub use futures;
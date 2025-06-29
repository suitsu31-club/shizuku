#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![doc = include_str!("../README.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/suitsu31-club/shizuku/refs/heads/main/assets/icon.svg")]

/// Core of Shizuku.
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

pub use kanau::*;
pub use kanau;

pub use core::message::*;
pub use core::error;
pub use core::error::Error;
pub use core::processor::*;

pub use tracing;
pub use futures;

#[cfg(feature = "json")]
/// Implement [MessageDe] by `serde_json` if it already implements `serde::Deserialize`.
pub use kanau_macro::JsonMessageDe;

#[cfg(feature = "json")]
/// Implement [MessageSer] by `serde_json` if it already implements `serde::Serialize`.
pub use kanau_macro::JsonMessageSer;

#[cfg(feature = "protobuf")]
/// Implement [MessageDe] by `prost` if it already implements `prost::Message`.
pub use kanau_macro::ProstMessageDe;

#[cfg(feature = "protobuf")]
/// Implement [MessageSer] by `prost` if it already implements `prost::Message`.
pub use kanau_macro::ProstMessageSer;

#[cfg(feature = "bincode")]
/// Implement [MessageDe] by `bincode` if it already implements `bincode::Decode`.
pub use kanau_macro::BincodeMessageDe;

#[cfg(feature = "bincode")]
/// Implement [MessageSer] by `bincode` if it already implements `bincode::Encode`.
pub use kanau_macro::BincodeMessageSer;

/// reexports of essentials
pub mod prelude {
    pub use crate::core::error::Error;
    pub use kanau::processor::{Processor, FinalProcessor};
    pub use kanau::flow::EarlyReturn;
    pub use kanau::early_return;
    pub use kanau::message::{MessageDe, MessageSer};
    pub use crate::core::message::{
        StaticSubjectMessage, 
        JetStreamMessageSendTrait
    };
    pub use crate::kv::{KeyValue, KeyValueRead, KeyValueWrite};
}
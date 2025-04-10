/// Internal error types.
pub mod error;

/// Define the message struct using inside `ame-bus`.
pub mod message;

/// Abstract layer for processing the message.
pub mod processor;

pub use processor::*;
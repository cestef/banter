//! Drive an [`App`](crate::App) in-process, for tests and reference clients.
//! Nothing here touches the network.

mod client;
mod pipe;
mod script;

pub use client::Client;
pub use script::Script;

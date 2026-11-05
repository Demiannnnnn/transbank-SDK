//! Asynchronous API based on Tokio (feature `tokio`).
//!
//! Provides [`AsyncLink`] and [`AsyncPosIntegrado`], which reuse the sans-IO protocol
//! parsing and encoding from [`protocol`](crate::protocol) and [`message`](crate::message)
//! without spawning background tasks or imposing blocking threads.

mod client;
mod link;

pub use client::AsyncPosIntegrado;
pub use link::AsyncLink;

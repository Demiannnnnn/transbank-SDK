//! Asynchronous API based on Tokio (feature `tokio`).
//!
//! Provides [`AsyncLink`] and [`AsyncPosIntegrado`], which reuse the sans-IO protocol
//! parsing and encoding from [`protocol`](crate::protocol) and [`message`](crate::message)
//! without spawning background tasks or imposing blocking threads.
//!
//! # Runtime requirements
//!
//! All asynchronous operations must run within an active Tokio runtime with the time
//! driver enabled (e.g. `enable_time()`), as well as the I/O driver enabled for
//! network sockets or serial streams (such as `enable_all()`).

mod client;
mod link;

pub use client::AsyncPosIntegrado;
pub use link::AsyncLink;

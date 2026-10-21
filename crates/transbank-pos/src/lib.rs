//! # transbank-pos
//!
//! **Unofficial — not affiliated with Transbank S.A.** This crate is an independent
//! implementation of the serial protocol used by Transbank POS terminals
//! (POS Integrado and POS Autoservicio), written from the public sources of the
//! official SDKs. "Transbank" is a trademark of its owner and is used here only to
//! describe compatibility.
//!
//! The crate is layered so that every part can be tested without hardware:
//!
//! - [`protocol`]: sans-IO framing (`STX payload ETX LRC`), LRC and decoding.
//! - [`transport`]: byte channels to the terminal (memory, TCP and, with the
//!   `serial` feature, a serial port).
//! - Link and clients (`Link`, `PosIntegrado`): ACK/NAK handling, timeouts and
//!   typed commands.
//!
//! The wire protocol is described in
//! [`docs/protocol-spec.md`](https://github.com/Demiannnnnn/transbank-SDK/blob/develop/docs/protocol-spec.md).
//!
//! ## Example
//!
//! A sale against the simulator of the `transbank-pos-sim` crate. With a real
//! terminal the transport is a serial port (`serial::SerialTransport`) or a TCP
//! bridge, and the clock is the system one (`PosIntegrado::new`).
//!
//! ```
//! use transbank_pos::{LinkConfig, PosIntegrado, SaleRequest, SalesDetailMode};
//! use transbank_pos::transport::ManualClock;
//! use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};
//!
//! let clock = ManualClock::new();
//! let transport = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
//! let mut pos = PosIntegrado::with_clock(transport, LinkConfig::default(), clock);
//!
//! pos.poll()?;
//! let sale = pos.sale_with_status(&SaleRequest::new(12_000, "ABC123")?, |status| {
//!     // "Esperando Leer Tarjeta", "Solicitando Ingreso de Clave"…
//!     let _ = status.message();
//! })?;
//! assert!(sale.is_approved());
//!
//! let detail = pos.sales_detail(SalesDetailMode::SendToHost)?;
//! assert_eq!(detail.len(), 1);
//! assert!(pos.close()?.is_approved());
//! # Ok::<(), transbank_pos::Error>(())
//! ```
//!
//! ## Status
//!
//! Early development: the API is not stable and the crate has not been validated
//! against a real terminal yet.
//!
//! ## Features
//!
//! - `serial` (default): serial port transport, port listing and auto-detection.
//! - `serde`: `Serialize`/`Deserialize` for requests, responses and domain types.
//! - `tracing`: link-level `tracing` events (sensitive fields redacted).

#![forbid(unsafe_code)]

mod client;
mod codes;
mod error;
mod link;
pub mod message;
pub mod protocol;
#[cfg(feature = "serial")]
pub mod serial;
pub mod transport;

pub use client::PosIntegrado;
pub use codes::{IntermediateStatus, ResponseCode};
pub use error::{Error, Result};
pub use link::{Link, LinkConfig};
pub use message::{
    CloseResponse, DetailEntry, LoadKeysResponse, MultiCodeDetailEntry, MultiCodeSaleRequest,
    MultiCodeSaleResponse, RefundResponse, SaleRequest, SaleResponse, SalesDetailMode,
    TotalsResponse,
};
pub use protocol::types::{
    Amount, CardBrand, CardType, CommerceCode, OperationNumber, PosDate, PosTime, Ticket,
};
pub use transport::{Clock, SystemClock, Transport};

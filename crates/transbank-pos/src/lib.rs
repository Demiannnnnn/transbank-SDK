//! # transbank-pos
//!
//! **Unofficial — not affiliated with, endorsed or supported by Transbank S.A.**
//! "Transbank" is a trademark of its owner and is used here only to describe
//! compatibility. The protocol was derived from the public sources of the official
//! SDKs (BSD-3-Clause, see `NOTICE`). Always validate with your own terminal before
//! charging real money.
//!
//! Rust SDK for the **Transbank POS terminals** of Chile connected by USB or serial
//! port: framing (`STX payload ETX LRC`), link handling (ACK/NAK, retries, timeouts)
//! and typed commands. Every layer can be tested without hardware.
//!
//! | Layer | What |
//! |:--|:--|
//! | [`protocol`] | sans-IO frame encoder/decoder, LRC, function codes |
//! | [`message`] | request payloads and typed response parsers |
//! | [`Link`] | ACK/NAK, retransmission of unacknowledged frames, timeouts |
//! | [`PosIntegrado`] | the client: one method per operation |
//! | [`transport`], `serial` | TCP, in-memory and (feature `serial`) serial port transports |
//!
//! **Terminals:** POS Integrado (Verifone VX520/VX520c, Ingenico Desk 3500, as cited
//! by the official SDKs). POS Autoservicio is planned. The crate has **not yet been
//! validated against a real terminal**; see the open questions of the protocol
//! specification.
//!
//! ## Quick start
//!
//! With a real terminal:
//!
//! ```no_run
//! use transbank_pos::{PosIntegrado, SaleRequest};
//!
//! let mut pos = PosIntegrado::open("/dev/ttyACM0")?; // COM3 on Windows
//! pos.poll()?;
//! let sale = pos.sale(&SaleRequest::new(12_000, "ABC123")?)?;
//! if sale.is_approved() {
//!     // authorization_code, operation_number, card_brand, installments…
//! } else {
//!     // a declined card is a response, not an error
//!     let _reason = sale.response_code.message();
//! }
//! # Ok::<(), transbank_pos::Error>(())
//! ```
//!
//! Without hardware, with the simulator of the `transbank-pos-sim` crate (or its
//! `tbk-pos-sim` TCP server and [`transport::TcpTransport`]):
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
//! ## Operations
//!
//! | Method | Command | Response |
//! |:--|:--|:--|
//! | [`poll`](PosIntegrado::poll) | `0100` | ACK |
//! | [`load_keys`](PosIntegrado::load_keys) | `0800` | [`LoadKeysResponse`] |
//! | [`sale`](PosIntegrado::sale), [`sale_with_status`](PosIntegrado::sale_with_status) | `0200` | [`SaleResponse`] |
//! | [`multicode_sale`](PosIntegrado::multicode_sale) | `0270` | [`MultiCodeSaleResponse`] |
//! | [`last_sale`](PosIntegrado::last_sale) | `0250` | [`SaleResponse`] |
//! | [`multicode_last_sale`](PosIntegrado::multicode_last_sale) | `0280` | [`MultiCodeSaleResponse`] |
//! | [`refund`](PosIntegrado::refund) | `1200` | [`RefundResponse`] |
//! | [`totals`](PosIntegrado::totals) | `0700` | [`TotalsResponse`] |
//! | [`sales_detail`](PosIntegrado::sales_detail) | `0260` | `Vec<`[`DetailEntry`]`>` |
//! | [`multicode_sales_detail`](PosIntegrado::multicode_sales_detail) | `0290` | `Vec<`[`MultiCodeDetailEntry`]`>` |
//! | [`close`](PosIntegrado::close) | `0500` | [`CloseResponse`] |
//! | [`set_normal_mode`](PosIntegrado::set_normal_mode) | `0300` | ACK |
//!
//! `Err` is reserved for communication and protocol failures ([`Error`]); every
//! response has `is_approved()` and a [`ResponseCode`] with the message of the POS.
//!
//! ## Avoiding double charges
//!
//! The link retransmits a frame only while the POS has **not** acknowledged it. Once
//! a sale or refund is acknowledged it is never written again. If the response then
//! does not arrive ([`Error::ResponseTimeout`], an I/O error), **the result is
//! unknown**: the customer may have paid. Never resend the sale blindly; call
//! [`PosIntegrado::last_sale`] and compare ticket and amount (see the example in
//! [`PosIntegrado::sale`]).
//!
//! ## Features
//!
//! - `serial` (default): [`serial::SerialTransport`], port listing and
//!   [`PosIntegrado::autoconnect`].
//! - `serde`: `Serialize` for responses and domain types, `Deserialize` for requests.
//! - `tracing`: link events on the `transbank_pos::link` target; payloads only at
//!   `trace` level, with account numbers and vouchers redacted.
//!
//! The crate has no `unsafe` code, never panics on input from the POS, uses integer
//! CLP amounts and spawns no threads.
//!
//! ## MSRV and license
//!
//! Rust 1.85 (edition 2024). Licensed under either of Apache License 2.0 or MIT
//! license at your option. Protocol specification:
//! [`docs/protocol-spec.md`](https://github.com/Demiannnnnn/transbank-SDK/blob/main/docs/protocol-spec.md).

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

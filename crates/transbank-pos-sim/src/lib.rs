//! # transbank-pos-sim
//!
//! **Unofficial — not affiliated with Transbank S.A.** Simulator of a Transbank POS
//! terminal for testing applications built on `transbank-pos` without hardware
//! (`docs/simulator-spec.md` in the repository):
//!
//! - [`PosSimulator`]: sans-IO fake POS with injectable failures ([`Scenario`]).
//! - [`SimTransport`]: the simulator as a `transbank_pos::Transport` with simulated time.
//! - [`TcpServer`]: the simulator over TCP (also the `tbk-pos-sim` binary).
//!
//! The simulator is deterministic: fixed date and time, authorization codes derived
//! from the operation number, no randomness. POS Autoservicio is pending (AUTO-01).

#![forbid(unsafe_code)]

mod request;
mod scenario;
mod simulator;
mod tcp;
mod transport;

pub use request::{SaleArgs, SimRequest, parse_request};
pub use scenario::Scenario;
pub use simulator::{
    PosSimulator, SIM_COMMERCE_CODE, SIM_TERMINAL_ID, SimulatedSale, TerminalKind,
};
pub use tcp::TcpServer;
pub use transport::SimTransport;

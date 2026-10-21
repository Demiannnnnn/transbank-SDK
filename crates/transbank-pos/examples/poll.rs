//! Checks the connection with the POS (`0100`).
//!
//! Runs against the in-memory simulator; SERIAL-01 switches it to `TBK_POS_TCP` /
//! `TBK_POS_PORT`.

use std::io::Write;

use transbank_pos::transport::ManualClock;
use transbank_pos::{LinkConfig, PosIntegrado};
use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ManualClock::new();
    let transport = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
    let mut pos = PosIntegrado::with_clock(transport, LinkConfig::default(), clock);
    pos.poll()?;
    writeln!(std::io::stdout().lock(), "POS conectado")?;
    Ok(())
}

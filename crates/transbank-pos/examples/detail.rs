//! Sales detail sent to the host, then totals.
//!
//! Runs against the in-memory simulator (with two sales); SERIAL-01 switches it to
//! `TBK_POS_TCP` / `TBK_POS_PORT`.

use std::io::Write;

use transbank_pos::transport::ManualClock;
use transbank_pos::{LinkConfig, PosIntegrado, SaleRequest, SalesDetailMode};
use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ManualClock::new();
    let transport = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
    let mut pos = PosIntegrado::with_clock(transport, LinkConfig::default(), clock);
    pos.sale(&SaleRequest::new(12_000, "1")?)?;
    pos.sale(&SaleRequest::new(3_001, "2")?)?;

    let mut out = std::io::stdout().lock();
    for entry in pos.sales_detail(SalesDetailMode::SendToHost)? {
        writeln!(
            out,
            "ticket {:>6}  monto {:>9}  operación {}  {}",
            entry.ticket.unwrap_or_default(),
            entry.amount.unwrap_or_default(),
            entry.operation_number.unwrap_or_default(),
            entry
                .card_type
                .map(|t| t.code().to_owned())
                .unwrap_or_default()
        )?;
    }
    let totals = pos.totals()?;
    writeln!(
        out,
        "{} ventas, total {}",
        totals.tx_count.unwrap_or_default(),
        totals.tx_total.unwrap_or_default()
    )?;
    Ok(())
}

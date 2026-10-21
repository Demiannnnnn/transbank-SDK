//! Sales detail sent to the host, then totals (read-only: no sale is made).
//!
//! Connects to `TBK_POS_PORT` (serial port) or `TBK_POS_TCP` (default
//! `127.0.0.1:7777`): start the simulator with `cargo run -p transbank-pos-sim`.

use std::io::Write;
use std::time::Duration;

use transbank_pos::transport::TcpTransport;
use transbank_pos::{PosIntegrado, SalesDetailMode, Transport};

/// Connects to the POS: `TBK_POS_PORT` (serial port, feature `serial`) or
/// `TBK_POS_TCP` (default `127.0.0.1:7777`, the `tbk-pos-sim` simulator).
fn connect() -> Result<PosIntegrado<Box<dyn Transport>>, Box<dyn std::error::Error>> {
    #[cfg(feature = "serial")]
    if let Ok(port) = std::env::var("TBK_POS_PORT") {
        let transport = transbank_pos::serial::SerialTransport::open(
            &port,
            transbank_pos::serial::DEFAULT_BAUD_INTEGRADO,
        )?;
        return Ok(PosIntegrado::new(Box::new(transport)));
    }
    let address = std::env::var("TBK_POS_TCP").unwrap_or_else(|_| "127.0.0.1:7777".to_owned());
    let transport = TcpTransport::connect(address.as_str(), Duration::from_secs(2))?;
    Ok(PosIntegrado::new(Box::new(transport)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut pos = connect()?;

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

//! Checks the connection with the POS (`0100`).
//!
//! Connects to `TBK_POS_PORT` (serial port) or `TBK_POS_TCP` (default
//! `127.0.0.1:7777`): start the simulator with `cargo run -p transbank-pos-sim`.

use std::io::Write;
use std::time::Duration;

use transbank_pos::transport::TcpTransport;
use transbank_pos::{PosIntegrado, Transport};

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
    pos.poll()?;
    writeln!(std::io::stdout().lock(), "POS conectado")?;
    Ok(())
}

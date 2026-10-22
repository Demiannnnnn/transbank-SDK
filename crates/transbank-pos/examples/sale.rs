//! Sale with intermediate messages and recovery of an unknown result.
//!
//! Usage: `cargo run --example sale -- [amount] [ticket]`.
//!
//! Connects to `TBK_POS_PORT` (serial port) or `TBK_POS_TCP` (default
//! `127.0.0.1:7777`): start the simulator with `cargo run -p transbank-pos-sim`.

use std::io::Write;
use std::time::Duration;

use transbank_pos::transport::TcpTransport;
use transbank_pos::{Error, PosIntegrado, SaleRequest, Transport};

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
    let mut args = std::env::args().skip(1);
    let amount: u32 = args.next().map_or(Ok(12_000), |a| a.parse())?;
    let ticket = args.next().unwrap_or_else(|| "ABC123".to_owned());
    let request = SaleRequest::new(amount, &ticket)?;

    let mut pos = connect()?;
    let mut out = std::io::stdout().lock();

    match pos.sale_with_status(&request, |status| {
        let _ = writeln!(std::io::stdout().lock(), "  … {}", status.message());
    }) {
        Ok(sale) if sale.is_approved() => writeln!(
            out,
            "Aprobada: {} autorización {} operación {}",
            request.amount(),
            sale.authorization_code.unwrap_or_default(),
            sale.operation_number.unwrap_or_default()
        )?,
        Ok(sale) => writeln!(out, "Rechazada: {}", sale.response_code)?,
        Err(err @ (Error::ResponseTimeout { .. } | Error::Io(_))) => {
            // Unknown result: never resend blindly (ADR-04).
            writeln!(
                out,
                "Resultado desconocido ({err}); consultando la última venta…"
            )?;
            let last = pos.last_sale()?;
            let same = last.ticket.as_deref() == Some(request.ticket().as_str())
                && last.amount == Some(request.amount().get());
            writeln!(
                out,
                "Última venta: {} (¿es esta venta? {same})",
                last.response_code
            )?;
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}

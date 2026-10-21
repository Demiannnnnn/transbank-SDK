//! Sale with intermediate messages and recovery of an unknown result.
//!
//! Usage: `cargo run --example sale -- [amount] [ticket]`. Runs against the
//! in-memory simulator; SERIAL-01 switches it to `TBK_POS_TCP` / `TBK_POS_PORT`.

use std::io::Write;

use transbank_pos::transport::ManualClock;
use transbank_pos::{Error, LinkConfig, PosIntegrado, SaleRequest};
use transbank_pos_sim::{PosSimulator, SimTransport, TerminalKind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let amount: u32 = args.next().map_or(Ok(12_000), |a| a.parse())?;
    let ticket = args.next().unwrap_or_else(|| "ABC123".to_owned());
    let request = SaleRequest::new(amount, &ticket)?;

    let clock = ManualClock::new();
    let transport = SimTransport::new(PosSimulator::new(TerminalKind::Integrado), clock.clone());
    let mut pos = PosIntegrado::with_clock(transport, LinkConfig::default(), clock);
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

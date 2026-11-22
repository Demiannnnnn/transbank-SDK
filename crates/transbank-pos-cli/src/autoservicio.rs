//! Autoservicio command handler for `tbk-pos`.

use transbank_pos::{Clock, MultiCodeSaleRequest, PosAutoservicio, SaleRequest, Transport};

use crate::format;
use crate::json;
use crate::{Cli, Command, Failure, Outcome, Output, confirm, run_raw, say, unknown_result};

pub(crate) fn print_response_autoservicio(
    out: &Output,
    value: impl FnOnce() -> serde_json::Result<serde_json::Value>,
    text: &str,
    voucher: Option<&String>,
    lines: Option<Vec<&str>>,
    requested_voucher: bool,
) -> Result<(), Failure> {
    if out.json {
        let mut val = value()?;
        json::redact(&mut val, out.show_voucher);
        return say(&serde_json::to_string_pretty(&val)?);
    }
    say(text)?;
    if requested_voucher || out.show_voucher {
        if let Some(lines) = lines {
            for line in lines {
                say(line)?;
            }
        } else if let Some(raw) = voucher {
            if !raw.is_empty() {
                say(raw)?;
            }
        }
    }
    Ok(())
}

fn run_sale_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
    request: &SaleRequest,
    status: bool,
) -> Result<Outcome, Failure> {
    let result = if status {
        pos.sale_with_status(request, |s| out.status(&s))
    } else {
        pos.sale(request)
    };
    let sale = result.map_err(|err| unknown_result(cli, err))?;
    let text = format::format_self_service_sale("Venta", &sale);
    print_response_autoservicio(
        out,
        || serde_json::to_value(&sale),
        &text,
        sale.voucher.as_ref(),
        sale.voucher_lines(),
        request.voucher(),
    )?;
    Ok(Outcome::from_approved(sale.is_approved()))
}

fn run_multicode_sale_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
    request: &MultiCodeSaleRequest,
) -> Result<Outcome, Failure> {
    let sale = pos
        .multicode_sale(request)
        .map_err(|err| unknown_result(cli, err))?;
    let text = format::format_self_service_multicode_sale("Venta multicódigo", &sale);
    print_response_autoservicio(
        out,
        || serde_json::to_value(&sale),
        &text,
        sale.sale.voucher.as_ref(),
        sale.sale.voucher_lines(),
        request.sale().voucher(),
    )?;
    Ok(Outcome::from_approved(sale.is_approved()))
}

#[derive(Clone, Copy)]
struct MulticodeArgs<'a> {
    amount: u32,
    ticket: &'a str,
    commerce: &'a str,
    status: bool,
    voucher: bool,
}

fn run_multicode_cmd_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
    args: MulticodeArgs<'_>,
) -> Result<Outcome, Failure> {
    if args.status {
        return Err(Failure::Usage(
            "el Autoservicio no soporta mensajes intermedios en venta multicódigo".to_owned(),
        ));
    }
    let request = MultiCodeSaleRequest::new(args.amount, args.ticket, args.commerce)?
        .with_voucher(args.voucher);
    run_multicode_sale_autoservicio(cli, out, pos, &request)
}

fn run_last_sale_autoservicio<T: Transport, C: Clock>(
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
    voucher: bool,
) -> Result<Outcome, Failure> {
    let sale = pos.last_sale(voucher)?;
    let text = format::format_self_service_sale("Última venta", &sale);
    print_response_autoservicio(
        out,
        || serde_json::to_value(&sale),
        &text,
        sale.voucher.as_ref(),
        sale.voucher_lines(),
        voucher,
    )?;
    Ok(Outcome::from_approved(sale.is_approved()))
}

fn run_refund_last_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
) -> Result<Outcome, Failure> {
    if !confirm(cli, "Anular la última venta")? {
        say("Cancelado")?;
        return Ok(Outcome::NotApproved);
    }
    let refund = pos.refund_last()?;
    out.response(
        || serde_json::to_value(&refund),
        &format::format_refund(&refund),
        None,
    )?;
    Ok(Outcome::from_approved(refund.is_approved()))
}

fn run_close_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
    voucher: bool,
) -> Result<Outcome, Failure> {
    if !confirm(cli, "Cerrar el día en el POS")? {
        say("Cancelado")?;
        return Ok(Outcome::NotApproved);
    }
    let close = pos.close(voucher)?;
    let text = format::format_close(&close);
    print_response_autoservicio(
        out,
        || serde_json::to_value(&close),
        &text,
        close.voucher.as_ref(),
        close.voucher_lines(),
        voucher,
    )?;
    Ok(Outcome::from_approved(close.is_approved()))
}

fn run_load_keys_autoservicio<T: Transport, C: Clock>(
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
) -> Result<Outcome, Failure> {
    let keys = pos.load_keys()?;
    out.response(
        || serde_json::to_value(&keys),
        &format::format_load_keys(&keys),
        None,
    )?;
    Ok(Outcome::from_approved(keys.is_approved()))
}

fn run_init_autoservicio<T: Transport, C: Clock>(
    pos: &mut PosAutoservicio<T, C>,
) -> Result<Outcome, Failure> {
    pos.initialize()?;
    say("Inicialización enviada (comprueba con init-result)")?;
    Ok(Outcome::Ok)
}

fn run_init_result_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
) -> Result<Outcome, Failure> {
    let result = pos.initialization_result()?;
    let text = format::format_initialization(&result, cli.sim);
    out.response(|| serde_json::to_value(&result), &text, None)?;
    Ok(Outcome::from_approved(result.is_approved()))
}

pub(crate) fn run_autoservicio<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosAutoservicio<T, C>,
) -> Result<Outcome, Failure> {
    match &cli.command {
        Command::Ports | Command::Autoconnect => {
            Err(Failure::Usage("este comando no usa conexión".to_owned()))
        }
        Command::Poll => {
            pos.poll()?;
            say("POS conectado")?;
            Ok(Outcome::Ok)
        }
        Command::LoadKeys => run_load_keys_autoservicio(out, pos),
        Command::Init => run_init_autoservicio(pos),
        Command::InitResult => run_init_result_autoservicio(cli, out, pos),
        Command::Sale {
            amount,
            ticket,
            status,
            voucher,
        } => {
            let request = SaleRequest::new(*amount, ticket)?.with_voucher(*voucher);
            run_sale_autoservicio(cli, out, pos, &request, *status)
        }
        Command::MulticodeSale {
            amount,
            ticket,
            commerce,
            status,
            voucher,
        } => run_multicode_cmd_autoservicio(
            cli,
            out,
            pos,
            MulticodeArgs {
                amount: *amount,
                ticket,
                commerce,
                status: *status,
                voucher: *voucher,
            },
        ),
        Command::LastSale { voucher } => run_last_sale_autoservicio(out, pos, *voucher),
        Command::Refund { .. } => Err(Failure::Usage(
            "el Autoservicio solo anula la última venta: usa refund-last".to_owned(),
        )),
        Command::RefundLast => run_refund_last_autoservicio(cli, out, pos),
        Command::Totals => Err(Failure::Usage(
            "el Autoservicio no soporta totales".to_owned(),
        )),
        Command::Detail { .. } => Err(Failure::Usage(
            "el Autoservicio no soporta detalle de ventas".to_owned(),
        )),
        Command::Close { voucher } => run_close_autoservicio(cli, out, pos, *voucher),
        Command::NormalMode => Err(Failure::Usage(
            "el Autoservicio no soporta modo normal".to_owned(),
        )),
        Command::Raw { payload, expect } => run_raw(pos.link_mut(), payload, expect.as_deref()),
    }
}

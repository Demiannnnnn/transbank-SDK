//! `tbk-pos`: command-line tool to operate and diagnose Transbank POS terminals (unofficial).
//!
//! Exit codes: 0 approved/ok · 1 response not approved · 2 communication or protocol
//! error · 64 incorrect usage.

#![forbid(unsafe_code)]

mod format;
mod json;

use std::fs::File;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use transbank_pos::protocol::{FC_INTERMEDIATE, encode_frame};
use transbank_pos::serial::{self, DEFAULT_BAUD_INTEGRADO, SerialTransport};
use transbank_pos::transport::{ManualClock, TcpTransport};
use transbank_pos::{
    Clock, Error, IntermediateStatus, LinkConfig, MultiCodeSaleRequest, OperationNumber,
    PosIntegrado, SaleRequest, SalesDetailMode, Transport,
};
use transbank_pos_sim::{PosSimulator, Scenario, SimTransport, TerminalKind};

const EXAMPLES: &str = "Ejemplos:
  tbk-pos ports
  tbk-pos --port /dev/ttyACM0 poll
  tbk-pos --port COM3 sale 1500 T00001 --status
  tbk-pos --tcp 127.0.0.1:7777 last-sale
  tbk-pos --sim --scenario reject:1 sale 1000 T1
  tbk-pos --port /dev/ttyACM0 --trace-file hw.log detail

Herramienta NO oficial; no afiliada a Transbank.";

/// Herramienta NO oficial; no afiliada a Transbank (Unofficial Transbank POS
/// command-line tool — not affiliated with Transbank S.A.).
///
/// Opera y diagnostica un POS Integrado por puerto serie, TCP o con el simulador en
/// memoria (`--sim`).
#[derive(Debug, Parser)]
#[command(name = "tbk-pos", version, after_help = EXAMPLES)]
struct Cli {
    /// Puerto serie del POS (COM3, /dev/ttyACM0, /dev/cu.usbmodem…).
    #[arg(long, conflicts_with_all = ["tcp", "sim"])]
    port: Option<String>,
    /// Baudios del puerto serie.
    #[arg(long, default_value_t = DEFAULT_BAUD_INTEGRADO)]
    baud: u32,
    /// Conexión TCP (puente serie-red o `tbk-pos-sim`), p. ej. 127.0.0.1:7777.
    #[arg(long, conflicts_with = "sim")]
    tcp: Option<String>,
    /// Simulador en memoria (arranca con una venta de demostración, operación 000001).
    #[arg(long)]
    sim: bool,
    /// Escenario del simulador (approve, reject:CÓDIGO, intermediate, no-response,
    /// corrupt-lrc:N, require-close, drop-acks:N, nak:N, voucher:N).
    #[arg(long, requires = "sim")]
    scenario: Option<Scenario>,
    /// Espera del ACK, en milisegundos (por defecto 2000).
    #[arg(long, global = true)]
    ack_timeout_ms: Option<u64>,
    /// Espera de la respuesta, en segundos (por defecto 150).
    #[arg(long, global = true)]
    response_timeout_s: Option<u64>,
    /// Escribe la traza del enlace (redactada) en este archivo; filtro con `RUST_LOG`
    /// (por defecto `transbank_pos=trace`).
    #[arg(long, global = true)]
    trace_file: Option<PathBuf>,
    #[command(flatten)]
    flags: Flags,
    #[command(subcommand)]
    command: Command,
}

/// Output and confirmation flags.
#[derive(Debug, Args)]
struct Flags {
    /// Salida JSON (campos sensibles redactados).
    #[arg(long, global = true)]
    json: bool,
    /// Muestra el voucher (y lo incluye en el JSON).
    #[arg(long, global = true)]
    show_voucher: bool,
    /// No pide confirmación en anulación, cierre y modo normal.
    #[arg(long, global = true)]
    yes: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Lista los puertos serie del sistema.
    Ports,
    /// Busca el POS en los puertos serie (USB primero).
    Autoconnect,
    /// Comprueba la conexión (0100).
    Poll,
    /// Carga las llaves (0800).
    LoadKeys,
    /// Venta (0200).
    Sale {
        /// Monto en pesos (50 a 999999999).
        amount: u32,
        /// Número de boleta/ticket (1 a 6 caracteres alfanuméricos).
        ticket: String,
        /// Muestra los mensajes intermedios del POS.
        #[arg(long)]
        status: bool,
        /// Pide el voucher en la respuesta.
        #[arg(long)]
        voucher: bool,
    },
    /// Venta multicódigo (0270).
    MulticodeSale {
        /// Monto en pesos.
        amount: u32,
        /// Número de boleta/ticket.
        ticket: String,
        /// Código de comercio hijo ("0" = principal).
        commerce: String,
        /// Muestra los mensajes intermedios del POS.
        #[arg(long)]
        status: bool,
        /// Pide el voucher en la respuesta.
        #[arg(long)]
        voucher: bool,
    },
    /// Última venta (0250).
    LastSale,
    /// Anula una venta de crédito por número de operación (1200).
    Refund {
        /// Número de operación de la venta.
        operation: String,
    },
    /// Totales desde el último cierre (0700).
    Totals,
    /// Detalle de ventas (0260).
    Detail {
        /// Imprime el detalle en el POS en vez de enviarlo.
        #[arg(long)]
        print: bool,
    },
    /// Cierre del día (0500).
    Close,
    /// Saca al POS del modo integrado (0300).
    NormalMode,
    /// Envía un payload arbitrario e imprime la respuesta (diagnóstico).
    Raw {
        /// Payload sin STX/ETX/LRC, p. ej. '0200|100|T00002||0|1|'.
        payload: String,
        /// Código de función de la respuesta esperada (por defecto: código + 10).
        #[arg(long)]
        expect: Option<String>,
    },
}

/// Failure of a command, mapped to an exit code.
enum Failure {
    /// Incorrect usage (64).
    Usage(String),
    /// Communication or protocol error (2), with an optional hint.
    Pos(Error, Option<String>),
    /// Writing the output failed (2).
    Output(std::io::Error),
}

impl From<std::io::Error> for Failure {
    fn from(err: std::io::Error) -> Self {
        Self::Output(err)
    }
}

impl From<serde_json::Error> for Failure {
    fn from(err: serde_json::Error) -> Self {
        Self::Output(err.into())
    }
}

impl From<Error> for Failure {
    fn from(err: Error) -> Self {
        match err {
            Error::InvalidArgument { field, reason } => {
                Self::Usage(format!("argumento inválido {field}: {reason}"))
            }
            other => Self::Pos(other, None),
        }
    }
}

/// Result of a command that ran: approved/ok (0) or not approved (1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Ok,
    NotApproved,
}

impl Outcome {
    fn from_approved(approved: bool) -> Self {
        if approved {
            Self::Ok
        } else {
            Self::NotApproved
        }
    }
}

/// Output options shared by every command.
struct Output {
    json: bool,
    show_voucher: bool,
}

fn say(text: &str) -> Result<(), Failure> {
    writeln!(std::io::stdout().lock(), "{text}")?;
    Ok(())
}

impl Output {
    /// Prints `value` as JSON (redacted) or the human `text`, plus the voucher lines
    /// with `--show-voucher`.
    fn response(
        &self,
        value: impl FnOnce() -> serde_json::Result<serde_json::Value>,
        text: &str,
        voucher: Option<Vec<&str>>,
    ) -> Result<(), Failure> {
        if self.json {
            let mut value = value()?;
            json::redact(&mut value, self.show_voucher);
            return say(&serde_json::to_string_pretty(&value)?);
        }
        say(text)?;
        if self.show_voucher {
            for line in voucher.unwrap_or_default() {
                say(line)?;
            }
        }
        Ok(())
    }

    /// Prints an intermediate status live (stderr with `--json`, to keep stdout valid).
    fn status(&self, status: &IntermediateStatus) {
        let text = format!("  … {}", status.message());
        let _ = if self.json {
            writeln!(std::io::stderr().lock(), "{text}")
        } else {
            writeln!(std::io::stdout().lock(), "{text}")
        };
    }
}

fn link_config(cli: &Cli) -> LinkConfig {
    let mut config = LinkConfig::default();
    if let Some(ms) = cli.ack_timeout_ms {
        config.ack_timeout = Duration::from_millis(ms);
    }
    if let Some(s) = cli.response_timeout_s {
        config.response_timeout = Duration::from_secs(s);
    }
    config
}

/// The connection flags, to suggest the next command to the user.
fn connection_flags(cli: &Cli) -> String {
    if let Some(port) = &cli.port {
        format!("--port {port}")
    } else if let Some(tcp) = &cli.tcp {
        format!("--tcp {tcp}")
    } else {
        "--sim".to_owned()
    }
}

fn confirm(cli: &Cli, action: &str) -> Result<bool, Failure> {
    if cli.flags.yes {
        return Ok(true);
    }
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return Err(Failure::Usage(format!(
            "{action} requiere confirmación: usa --yes si no hay terminal"
        )));
    }
    write!(std::io::stdout().lock(), "{action}. ¿Seguro? [s/N] ")?;
    std::io::stdout().lock().flush()?;
    let mut answer = String::new();
    stdin.lock().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "si" | "sí" | "y" | "yes"
    ))
}

/// Simulator in memory with one demo sale (operation 000001), so that `last-sale`,
/// `refund 1`, `detail` and `totals` have data in a fresh process.
fn demo_simulator(scenario: Option<Scenario>) -> PosSimulator {
    let mut sim = PosSimulator::new(TerminalKind::Integrado);
    if let Ok(frame) = encode_frame("0200|000012000|DEMO01||0|0") {
        sim.feed(&frame);
        sim.feed(&[transbank_pos::protocol::ACK]);
    }
    sim.with_scenario(scenario.unwrap_or_default())
}

fn sale_unknown_hint(cli: &Cli) -> String {
    format!(
        "Resultado desconocido: NO repitas la venta; ejecuta `tbk-pos {} last-sale`",
        connection_flags(cli)
    )
}

fn run_sale<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosIntegrado<T, C>,
    request: &SaleRequest,
    status: bool,
) -> Result<Outcome, Failure> {
    let result = if status {
        pos.sale_with_status(request, |s| out.status(&s))
    } else {
        pos.sale(request)
    };
    let sale = result.map_err(|err| unknown_result(cli, err))?;
    out.response(
        || serde_json::to_value(&sale),
        &format::format_sale("Venta", &sale),
        sale.voucher_lines(),
    )?;
    Ok(Outcome::from_approved(sale.is_approved()))
}

/// After the sale was sent, a timeout or I/O error leaves the result unknown (ADR-04).
fn unknown_result(cli: &Cli, err: Error) -> Failure {
    match err {
        Error::ResponseTimeout { .. } | Error::Io(_) | Error::BadLrc { .. } => {
            Failure::Pos(err, Some(sale_unknown_hint(cli)))
        }
        other => other.into(),
    }
}

fn run_multicode_sale<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosIntegrado<T, C>,
    request: &MultiCodeSaleRequest,
    status: bool,
) -> Result<Outcome, Failure> {
    let result = if status {
        pos.multicode_sale_with_status(request, |s| out.status(&s))
    } else {
        pos.multicode_sale(request)
    };
    let sale = result.map_err(|err| unknown_result(cli, err))?;
    let text = format::format_multicode_sale("Venta multicódigo", &sale);
    out.response(
        || serde_json::to_value(&sale),
        &text,
        sale.sale.voucher_lines(),
    )?;
    Ok(Outcome::from_approved(sale.is_approved()))
}

fn run_raw<T: Transport, C: Clock>(
    pos: &mut PosIntegrado<T, C>,
    payload: &str,
    expect: Option<&str>,
) -> Result<Outcome, Failure> {
    let function_code = payload.split('|').next().unwrap_or_default().trim();
    let expected = match expect {
        Some(code) => code.to_owned(),
        None if matches!(function_code, "0100" | "0300" | "0070") => {
            pos.link_mut().send_command(payload)?;
            say("ACK recibido")?;
            return Ok(Outcome::Ok);
        }
        None => function_code
            .parse::<u16>()
            .map(|code| format!("{:04}", code.saturating_add(10)))
            .map_err(|_| Failure::Usage("payload sin código de función numérico".to_owned()))?,
    };
    // The link API takes a `&'static str`; leaking one short string per process is fine.
    let expected: &'static str = Box::leak(expected.into_boxed_str());
    let response = pos.link_mut().request(payload, expected, &mut |status| {
        let _ = writeln!(
            std::io::stdout().lock(),
            "<- {FC_INTERMEDIATE} {}",
            status.message()
        );
    })?;
    say(&format!("<- {response}"))?;
    Ok(Outcome::Ok)
}

fn run_detail<T: Transport, C: Clock>(
    out: &Output,
    pos: &mut PosIntegrado<T, C>,
    print: bool,
) -> Result<Outcome, Failure> {
    if print {
        pos.sales_detail(SalesDetailMode::PrintOnPos)?;
        say("Detalle enviado a la impresora del POS")?;
        return Ok(Outcome::Ok);
    }
    let entries = pos.sales_detail(SalesDetailMode::SendToHost)?;
    if out.json {
        return out
            .response(|| serde_json::to_value(&entries), "", None)
            .map(|()| Outcome::Ok);
    }
    if entries.is_empty() {
        say("Sin ventas desde el último cierre")?;
    }
    for entry in &entries {
        say(&format::format_detail_entry(entry))?;
    }
    Ok(Outcome::Ok)
}

fn run<T: Transport, C: Clock>(
    cli: &Cli,
    out: &Output,
    pos: &mut PosIntegrado<T, C>,
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
        Command::LoadKeys => {
            let keys = pos.load_keys()?;
            out.response(
                || serde_json::to_value(&keys),
                &format::format_load_keys(&keys),
                None,
            )?;
            Ok(Outcome::from_approved(keys.is_approved()))
        }
        Command::Sale {
            amount,
            ticket,
            status,
            voucher,
        } => {
            let request = SaleRequest::new(*amount, ticket)?.with_voucher(*voucher);
            run_sale(cli, out, pos, &request, *status)
        }
        Command::MulticodeSale {
            amount,
            ticket,
            commerce,
            status,
            voucher,
        } => {
            let request =
                MultiCodeSaleRequest::new(*amount, ticket, commerce)?.with_voucher(*voucher);
            run_multicode_sale(cli, out, pos, &request, *status)
        }
        Command::LastSale => {
            let sale = pos.last_sale()?;
            out.response(
                || serde_json::to_value(&sale),
                &format::format_sale("Última venta", &sale),
                sale.voucher_lines(),
            )?;
            Ok(Outcome::from_approved(sale.is_approved()))
        }
        Command::Refund { operation } => {
            let operation: OperationNumber = operation.parse()?;
            if !confirm(cli, &format!("Anular la operación {:06}", operation.get()))? {
                say("Cancelado")?;
                return Ok(Outcome::NotApproved);
            }
            let refund = pos.refund(operation)?;
            out.response(
                || serde_json::to_value(&refund),
                &format::format_refund(&refund),
                None,
            )?;
            Ok(Outcome::from_approved(refund.is_approved()))
        }
        Command::Totals => {
            let totals = pos.totals()?;
            out.response(
                || serde_json::to_value(&totals),
                &format::format_totals(&totals),
                None,
            )?;
            Ok(Outcome::from_approved(totals.is_approved()))
        }
        Command::Detail { print } => run_detail(out, pos, *print),
        Command::Close => {
            if !confirm(cli, "Cerrar el día en el POS")? {
                say("Cancelado")?;
                return Ok(Outcome::NotApproved);
            }
            let close = pos.close()?;
            out.response(
                || serde_json::to_value(&close),
                &format::format_close(&close),
                close.voucher_lines(),
            )?;
            Ok(Outcome::from_approved(close.is_approved()))
        }
        Command::NormalMode => {
            if !confirm(cli, "Sacar al POS del modo integrado")? {
                say("Cancelado")?;
                return Ok(Outcome::NotApproved);
            }
            pos.set_normal_mode()?;
            say("POS en modo normal (reactívalo desde el menú del POS)")?;
            Ok(Outcome::Ok)
        }
        Command::Raw { payload, expect } => run_raw(pos, payload, expect.as_deref()),
    }
}

fn run_ports(out: &Output) -> Result<Outcome, Failure> {
    let ports = serial::list_ports()?;
    if out.json {
        return out
            .response(|| serde_json::to_value(&ports), "", None)
            .map(|()| Outcome::Ok);
    }
    if ports.is_empty() {
        say("No hay puertos serie")?;
    }
    for port in &ports {
        say(&format::format_port(port))?;
    }
    Ok(Outcome::Ok)
}

fn run_autoconnect(cli: &Cli, out: &Output) -> Result<Outcome, Failure> {
    let Some((_, port)) = PosIntegrado::<SerialTransport>::autoconnect(cli.baud)? else {
        say("No se encontró ningún POS")?;
        return Ok(Outcome::NotApproved);
    };
    let text = format!("POS encontrado: {}", format::format_port(&port));
    out.response(|| serde_json::to_value(&port), &text, None)?;
    Ok(Outcome::Ok)
}

fn dispatch(cli: &Cli, out: &Output) -> Result<Outcome, Failure> {
    let config = link_config(cli);
    match &cli.command {
        Command::Ports => return run_ports(out),
        Command::Autoconnect => return run_autoconnect(cli, out),
        _ => {}
    }
    if let Some(port) = &cli.port {
        let transport = SerialTransport::open(port, cli.baud)?;
        run(cli, out, &mut PosIntegrado::with_config(transport, config))
    } else if let Some(address) = &cli.tcp {
        let transport = TcpTransport::connect(address.as_str(), Duration::from_secs(5))?;
        run(cli, out, &mut PosIntegrado::with_config(transport, config))
    } else if cli.sim {
        let clock = ManualClock::new();
        let transport = SimTransport::new(demo_simulator(cli.scenario.clone()), clock.clone());
        run(
            cli,
            out,
            &mut PosIntegrado::with_clock(transport, config, clock),
        )
    } else {
        Err(Failure::Usage(
            "falta la conexión: usa --port <puerto>, --tcp <host:puerto> o --sim".to_owned(),
        ))
    }
}

fn install_trace(path: &PathBuf) -> Result<(), Failure> {
    let file = File::create(path)?;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("transbank_pos=trace"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .try_init()
        .map_err(|err| Failure::Usage(format!("no se pudo activar la traza: {err}")))
}

fn report(failure: Failure) -> ExitCode {
    let mut stderr = std::io::stderr().lock();
    match failure {
        Failure::Usage(message) => {
            let _ = writeln!(stderr, "error: {message}");
            ExitCode::from(64)
        }
        Failure::Pos(err, hint) => {
            let _ = writeln!(stderr, "error: {err}");
            if let Some(hint) = hint {
                let _ = writeln!(stderr, "{hint}");
            }
            ExitCode::from(2)
        }
        Failure::Output(err) => {
            let _ = writeln!(stderr, "error de salida: {err}");
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return if err.use_stderr() {
                ExitCode::from(64)
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    if let Some(path) = &cli.trace_file {
        if let Err(failure) = install_trace(path) {
            return report(failure);
        }
    }
    let out = Output {
        json: cli.flags.json,
        show_voucher: cli.flags.show_voucher,
    };
    match dispatch(&cli, &out) {
        Ok(Outcome::Ok) => ExitCode::SUCCESS,
        Ok(Outcome::NotApproved) => ExitCode::from(1),
        Err(failure) => report(failure),
    }
}

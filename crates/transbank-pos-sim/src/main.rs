//! `tbk-pos-sim`: TCP server that simulates a Transbank POS terminal (unofficial).
//!
//! Ctrl-C ends the process with the default signal handling (no crate for signals):
//! the connection is closed by the operating system.

#![forbid(unsafe_code)]

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use transbank_pos::protocol::{FrameDecoder, LinkEvent, decode_latin1};
use transbank_pos_sim::{PosSimulator, Scenario, TcpServer, TerminalKind};

/// Simulador NO oficial de un POS de Transbank (Unofficial Transbank POS simulator —
/// not affiliated with Transbank S.A.).
///
/// Atiende un cliente TCP a la vez y responde como un POS Integrado, para probar
/// aplicaciones sin hardware.
#[derive(Debug, Parser)]
#[command(name = "tbk-pos-sim", version)]
struct Cli {
    /// Dirección de escucha.
    #[arg(long, default_value = "127.0.0.1:7777")]
    listen: String,
    /// Permite escuchar en una interfaz que no es loopback (sin autenticación).
    #[arg(long)]
    allow_remote: bool,
    /// Modelo de terminal.
    #[arg(long, value_enum, default_value_t = Kind::Integrado)]
    kind: Kind,
    /// Escenario: approve, reject:CÓDIGO, intermediate, no-response, corrupt-lrc:N,
    /// require-close, drop-acks:N, nak:N, voucher:LÍNEAS.
    #[arg(long, default_value = "approve")]
    scenario: Scenario,
    /// Demora antes de cada respuesta, en milisegundos (demos).
    #[arg(long, default_value_t = 0)]
    delay_ms: u64,
    /// No imprime los mensajes intercambiados.
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Kind {
    Integrado,
    Autoservicio,
}

/// Prints the payloads exchanged: `<-` from the host, `->` from the POS.
struct Printer {
    host: FrameDecoder,
    pos: FrameDecoder,
    events: Vec<LinkEvent>,
}

impl Printer {
    fn print(&mut self, input: &[u8], output: &[u8]) {
        let mut stdout = std::io::stdout().lock();
        self.host.push(input, &mut self.events);
        for event in self.events.drain(..) {
            let _ = writeln!(stdout, "<- {}", describe(&event));
        }
        self.pos.push(output, &mut self.events);
        for event in self.events.drain(..) {
            let _ = writeln!(stdout, "-> {}", describe(&event));
        }
    }
}

fn describe(event: &LinkEvent) -> String {
    match event {
        LinkEvent::Ack => "ACK".to_owned(),
        LinkEvent::Nak => "NAK".to_owned(),
        LinkEvent::Frame(payload) => decode_latin1(payload),
        LinkEvent::BadLrc { payload, .. } => format!("{} (LRC inválido)", decode_latin1(payload)),
        LinkEvent::Garbage(count) => format!("{count} bytes basura"),
        _ => "trama demasiado larga".to_owned(),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let kind = match cli.kind {
        Kind::Integrado => TerminalKind::Integrado,
        Kind::Autoservicio => TerminalKind::Autoservicio,
    };
    let mut sim = PosSimulator::new(kind).with_scenario(cli.scenario);
    let server = match TcpServer::bind(cli.listen.as_str(), cli.allow_remote) {
        Ok(server) => server.with_delay(Duration::from_millis(cli.delay_ms)),
        Err(err) => {
            let _ = writeln!(std::io::stderr().lock(), "error: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut server = server;
    let address = server
        .local_addr()
        .map_or_else(|_| cli.listen.clone(), |addr| addr.to_string());
    {
        let mut stdout = std::io::stdout().lock();
        if cli.allow_remote {
            let _ = writeln!(
                stdout,
                "AVISO: escuchando en {address} sin autenticación (--allow-remote)"
            );
        }
        let _ = writeln!(
            stdout,
            "Simulador NO oficial de POS Transbank escuchando en {address}"
        );
    }
    let mut printer = Printer {
        host: FrameDecoder::new(),
        pos: FrameDecoder::new(),
        events: Vec::new(),
    };
    loop {
        let result = server.serve_one_with(&mut sim, &mut |input, output| {
            if !cli.quiet {
                printer.print(input, output);
            }
        });
        if let Err(err) = result {
            let _ = writeln!(std::io::stderr().lock(), "error: {err}");
            return ExitCode::FAILURE;
        }
        if !cli.quiet {
            let _ = writeln!(std::io::stdout().lock(), "cliente desconectado");
        }
    }
}

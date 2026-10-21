//! `tbk-pos`: command-line tool to operate and diagnose Transbank POS terminals (unofficial).

#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

/// Unofficial Transbank POS command-line tool — not affiliated with Transbank S.A.
///
/// Operates and diagnoses POS Integrado / Autoservicio terminals over a serial
/// port, TCP or the built-in simulator.
#[derive(Debug, Parser)]
#[command(name = "tbk-pos", version)]
struct Cli {}

fn main() -> ExitCode {
    let _cli = Cli::parse();
    match Cli::command().print_help() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

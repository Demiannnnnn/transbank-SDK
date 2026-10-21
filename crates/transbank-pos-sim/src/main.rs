//! `tbk-pos-sim`: TCP server that simulates a Transbank POS terminal (unofficial).

#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

/// Unofficial Transbank POS simulator — not affiliated with Transbank S.A.
///
/// Simulates a POS Integrado / Autoservicio terminal over TCP so applications can
/// be tested without hardware.
#[derive(Debug, Parser)]
#[command(name = "tbk-pos-sim", version)]
struct Cli {}

fn main() -> ExitCode {
    let _cli = Cli::parse();
    match Cli::command().print_help() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

//! CLI tests in Autoservicio mode (`--autoservicio`).

use std::fs;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_tbk-pos"))
        .args(args)
        .stdin(Stdio::null())
        .output()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_code(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
}

#[test]
fn approved_sale() {
    let output = run(&["--autoservicio", "--sim", "sale", "1000", "T1"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(
        text.contains("[SIMULADOR] Venta APROBADA"),
        "text was: {text}"
    );
}

#[test]
fn rejected_sale_exits_one() {
    let output = run(&[
        "--autoservicio",
        "--sim",
        "--scenario",
        "reject:1",
        "sale",
        "1000",
        "T1",
    ])
    .unwrap();
    assert_code(&output, 1);
    let text = stdout(&output);
    assert!(
        text.contains("Venta RECHAZADA — 01 Rechazado"),
        "text was: {text}"
    );
}

#[test]
fn init_and_init_result() {
    let output = run(&["--autoservicio", "--sim", "init"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("Inicialización enviada"));

    let output = run(&["--autoservicio", "--sim", "init-result"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(
        text.contains("[SIMULADOR] Inicialización APROBADA"),
        "text was: {text}"
    );
}

#[test]
fn sale_with_voucher_prints_lines_of_forty_chars() {
    let output = run(&["--autoservicio", "--sim", "sale", "1000", "T1", "--voucher"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("SIMULADOR "))
        .collect();
    assert_eq!(lines.len(), 3);
    for line in lines {
        assert_eq!(line.len(), 40, "line was: {line:?}");
    }
}

#[test]
fn json_voucher_redaction_and_show_voucher() {
    // --json --voucher redacts
    let output = run(&[
        "--autoservicio",
        "--sim",
        "--json",
        "sale",
        "1000",
        "T1",
        "--voucher",
    ])
    .unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(
        text.contains("\"voucher\": \"<redacted 120 chars>\""),
        "{text}"
    );
    assert!(!text.contains("SIMULADOR 01"));

    // --json --show-voucher shows
    let output = run(&[
        "--autoservicio",
        "--sim",
        "--json",
        "--show-voucher",
        "sale",
        "1000",
        "T1",
        "--voucher",
    ])
    .unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(text.contains("SIMULADOR 01"), "{text}");
    assert!(!text.contains("<redacted 120 chars>"));
}

#[test]
fn refund_last_with_yes() {
    let output = run(&["--autoservicio", "--sim", "refund-last", "--yes"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("Anulación APROBADA"));
}

#[test]
fn exit_64_integrado_commands_without_autoservicio() {
    let commands = [
        vec!["--sim", "init"],
        vec!["--sim", "init-result"],
        vec!["--sim", "refund-last"],
        vec!["--sim", "last-sale", "--voucher"],
        vec!["--sim", "close", "--voucher"],
    ];
    for cmd in commands {
        let output = run(&cmd).unwrap();
        assert_code(&output, 64);
        assert!(
            stderr(&output).contains("solo está disponible con --autoservicio"),
            "stderr was: {}",
            stderr(&output)
        );
    }
}

#[test]
fn exit_64_autoservicio_commands_unsupported() {
    // refund with operation
    let output = run(&["--autoservicio", "--sim", "refund", "1", "--yes"]).unwrap();
    assert_code(&output, 64);
    assert!(
        stderr(&output).contains("el Autoservicio solo anula la última venta: usa refund-last")
    );

    // totals
    let output = run(&["--autoservicio", "--sim", "totals"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("el Autoservicio no soporta totales"));

    // detail
    let output = run(&["--autoservicio", "--sim", "detail"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("el Autoservicio no soporta detalle de ventas"));

    // normal-mode
    let output = run(&["--autoservicio", "--sim", "normal-mode", "--yes"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("el Autoservicio no soporta modo normal"));

    // multicode-sale --status
    let output = run(&[
        "--autoservicio",
        "--sim",
        "multicode-sale",
        "1000",
        "T1",
        "597012345678",
        "--status",
    ])
    .unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("el Autoservicio no soporta mensajes intermedios"));
}

#[test]
fn no_response_suggests_autoservicio_and_last_sale() {
    let output = run(&[
        "--autoservicio",
        "--sim",
        "--ack-timeout-ms",
        "50",
        "--response-timeout-s",
        "1",
        "--scenario",
        "no-response",
        "sale",
        "1000",
        "T1",
    ])
    .unwrap();
    assert_code(&output, 2);
    let err = stderr(&output);
    assert!(
        err.contains("tbk-pos --autoservicio --sim last-sale"),
        "stderr was: {err}"
    );
}

#[test]
fn voucher_and_show_voucher_does_not_duplicate_lines() {
    let output = run(&[
        "--autoservicio",
        "--sim",
        "--show-voucher",
        "sale",
        "1000",
        "T1",
        "--voucher",
    ])
    .unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    let count = text
        .lines()
        .filter(|line| line.starts_with("SIMULADOR "))
        .count();
    assert_eq!(count, 3, "voucher lines were duplicated: {text}");
}

#[test]
fn exit_64_without_connection() {
    let output = run(&["--autoservicio", "totals"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("el Autoservicio no soporta totales"));

    let output = run(&["init"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("init solo está disponible con --autoservicio"));
}

struct TempFile(std::path::PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn trace_file_redaction_autoservicio() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tbk-cli-auto-trace-{}.log", std::process::id()));
    let temp = TempFile(path.clone());
    let path_text = path.to_string_lossy().into_owned();

    let output = run(&[
        "--autoservicio",
        "--sim",
        "--trace-file",
        &path_text,
        "sale",
        "1000",
        "T1",
        "--voucher",
    ])
    .unwrap();
    assert_code(&output, 0);

    assert!(path.exists());
    let contents = fs::read_to_string(&path).unwrap();
    drop(temp);

    assert!(contents.contains("<redacted "));
    assert!(!contents.contains("SIMULADOR 01"));
}

#[test]
fn raw_commands_init_and_result() {
    let output = run(&["--autoservicio", "--sim", "raw", "0080"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("1080|90"));

    let output = run(&["--autoservicio", "--sim", "raw", "0070"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("ACK"));
}

#[test]
fn multicode_sale_approved_with_commerce() {
    let output = run(&[
        "--autoservicio",
        "--sim",
        "multicode-sale",
        "1000",
        "T1",
        "597012345678",
    ])
    .unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(text.contains("Venta multicódigo APROBADA"));
    assert!(text.contains("597012345678"));
}

#[test]
fn last_sale_and_close_with_voucher_print_lines_of_forty_chars() {
    let output = run(&["--autoservicio", "--sim", "last-sale", "--voucher"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("SIMULADOR "))
        .collect();
    assert_eq!(lines.len(), 3);
    for line in lines {
        assert_eq!(line.len(), 40, "line was: {line:?}");
    }

    let output = run(&["--autoservicio", "--sim", "close", "--voucher", "--yes"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("SIMULADOR "))
        .collect();
    assert_eq!(lines.len(), 3);
    for line in lines {
        assert_eq!(line.len(), 40, "line was: {line:?}");
    }
}

#[test]
fn corrupt_lrc_exits_two_with_autoservicio_hint() {
    let output = run(&[
        "--autoservicio",
        "--sim",
        "--scenario",
        "corrupt-lrc:9",
        "sale",
        "1000",
        "T1",
    ])
    .unwrap();
    assert_code(&output, 2);
    let err = stderr(&output);
    assert!(
        err.contains("--autoservicio"),
        "stderr did not contain --autoservicio: {err}"
    );
}

//! The real `tbk-pos` binary against the in-memory simulator (`--sim`).

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
fn poll() {
    let output = run(&["--sim", "poll"]).unwrap();
    assert_code(&output, 0);
    assert_eq!(stdout(&output), "POS conectado\n");
}

#[test]
fn approved_sale() {
    let output = run(&["--sim", "sale", "12000", "ABC123"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(
        text.starts_with("[SIMULADOR] Venta APROBADA — autorización "),
        "{text}"
    );
    assert!(
        text.contains("— $12.000 — VISA crédito ···") && text.contains("3 cuotas de $4.000"),
        "{text}"
    );
}

#[test]
fn sale_with_status_prints_intermediate_messages() {
    let output = run(&["--sim", "sale", "12000", "ABC123", "--status"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(text.contains("… Esperando Leer Tarjeta\n"), "{text}");
    assert!(text.contains("… Solicitando Ingreso de Clave\n"), "{text}");
}

#[test]
fn last_sale_totals_and_detail_of_the_demo_sale() {
    let output = run(&["--sim", "last-sale"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("Última venta APROBADA"));
    assert!(stdout(&output).contains("operación 000001"));

    let output = run(&["--sim", "totals"]).unwrap();
    assert_code(&output, 0);
    assert_eq!(stdout(&output), "1 venta — total $12.000\n");

    let output = run(&["--sim", "detail"]).unwrap();
    assert_code(&output, 0);
    assert!(
        stdout(&output).starts_with("ticket DEMO01"),
        "{}",
        stdout(&output)
    );

    let output = run(&["--sim", "detail", "--print"]).unwrap();
    assert_code(&output, 0);
}

#[test]
fn refund_and_close_with_yes() {
    let output = run(&["--sim", "refund", "1", "--yes"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("Anulación APROBADA"));

    let output = run(&["--sim", "close", "--yes"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("Cierre APROBADO"));
}

#[test]
fn declined_sale_exits_one() {
    let output = run(&["--sim", "--scenario", "reject:1", "sale", "100", "T1"]).unwrap();
    assert_code(&output, 1);
    assert!(stdout(&output).contains("Venta RECHAZADA — 01 Rechazado"));
}

#[test]
fn timeout_explains_unknown_result() {
    let output = run(&[
        "--sim",
        "--scenario",
        "no-response",
        "--response-timeout-s",
        "1",
        "sale",
        "100",
        "T1",
    ])
    .unwrap();
    assert_code(&output, 2);
    let text = stderr(&output);
    assert!(
        text.contains(
            "Resultado desconocido: NO repitas la venta; ejecuta `tbk-pos --sim last-sale`"
        ),
        "{text}"
    );
}

#[test]
fn json_is_valid_and_redacted() {
    let output = run(&["--sim", "--json", "sale", "12000", "ABC123"]).unwrap();
    assert_code(&output, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(value["amount"], 12_000);
    assert_eq!(value["ticket"], "ABC123");
    let account = value["account_number"].as_str().unwrap_or_default();
    assert!(account.starts_with("<redacted"), "{account}");
    assert!(!stdout(&output).contains("0000000000000000074"));
}

#[test]
fn show_voucher_prints_its_lines() {
    let output = run(&[
        "--sim",
        "--show-voucher",
        "sale",
        "12000",
        "ABC123",
        "--voucher",
    ])
    .unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("SIMULADOR 01 TICKET ABC123"));
}

#[test]
fn close_without_yes_and_without_tty_does_not_run() {
    let output = run(&["--sim", "close"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("--yes"));
    assert!(!stdout(&output).contains("Cierre"));
}

#[test]
fn missing_connection_is_usage_error() {
    let output = run(&["poll"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("--port"));
}

#[test]
fn invalid_arguments_are_usage_errors() {
    assert_code(&run(&["--sim", "sale", "10", "T1"]).unwrap(), 64);
    assert_code(&run(&["--sim", "sale", "100", "1234567"]).unwrap(), 64);
    assert_code(&run(&["--sim", "refund", "abc", "--yes"]).unwrap(), 64);
    assert_code(
        &run(&["--sim", "--tcp", "127.0.0.1:1", "poll"]).unwrap(),
        64,
    );
    assert_code(&run(&["--scenario", "approve", "poll"]).unwrap(), 64);
    assert_code(&run(&["frobnicate"]).unwrap(), 64);
}

#[test]
fn raw_payload_with_dotnet_format() {
    let output = run(&["--sim", "raw", "0200|100|T00002||0|1|"]).unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(text.contains("<- 0900 Esperando Leer Tarjeta"), "{text}");
    assert!(
        text.contains("<- 0210|00|597029414300|SIM00001|T00002|"),
        "{text}"
    );
}

#[test]
fn trace_file_is_redacted() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("tbk-pos-trace.log");
    let path_text = path.to_string_lossy().into_owned();
    let output = run(&[
        "--sim",
        "--trace-file",
        &path_text,
        "sale",
        "12000",
        "ABC123",
    ])
    .unwrap();
    assert_code(&output, 0);
    let trace = std::fs::read_to_string(&path).expect("trace file");
    assert!(trace.contains("frame sent"), "{trace}");
    assert!(trace.contains("<redacted 19>"), "{trace}");
    assert!(!trace.contains("0000000000000000074"), "{trace}");
}

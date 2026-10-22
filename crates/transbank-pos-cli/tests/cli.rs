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

#[test]
fn help_and_version() {
    let output = run(&["--help"]).unwrap();
    assert_code(&output, 0);
    let help = stdout(&output);
    assert!(help.contains("NO oficial"), "{help}");
    assert!(help.contains("Ejemplos:"), "{help}");
    for command in [
        "ports",
        "autoconnect",
        "sale",
        "multicode-sale",
        "last-sale",
        "refund",
        "detail",
        "close",
        "normal-mode",
        "raw",
    ] {
        assert!(help.contains(command), "{command} missing in help");
    }
    let output = run(&["--version"]).unwrap();
    assert_code(&output, 0);
    assert_eq!(
        stdout(&output),
        format!("tbk-pos {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn load_keys() {
    let output = run(&["--sim", "load-keys"]).unwrap();
    assert_code(&output, 0);
    assert_eq!(
        stdout(&output),
        "[SIMULADOR] Llaves cargadas — comercio 597029414300 — terminal SIM00001\n"
    );
}

#[test]
fn multicode_sale() {
    let output = run(&[
        "--sim",
        "multicode-sale",
        "5000",
        "M1",
        "597012345678",
        "--status",
    ])
    .unwrap();
    assert_code(&output, 0);
    let text = stdout(&output);
    assert!(text.contains("… Esperando Leer Tarjeta"), "{text}");
    assert!(text.contains("Venta multicódigo APROBADA"), "{text}");
    assert!(text.ends_with("— comercio 597012345678\n"), "{text}");
    assert_code(
        &run(&["--sim", "multicode-sale", "5000", "M1", "12a"]).unwrap(),
        64,
    );
}

#[test]
fn normal_mode_requires_confirmation() {
    assert_code(&run(&["--sim", "normal-mode"]).unwrap(), 64);
    let output = run(&["--sim", "normal-mode", "--yes"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).contains("modo normal"));
}

#[test]
fn refund_of_an_unknown_operation_exits_one() {
    let output = run(&["--sim", "refund", "99", "--yes"]).unwrap();
    assert_code(&output, 1);
    assert!(stdout(&output).contains("05 No existe Transacción para Anular"));
}

#[test]
fn require_close_scenario_exits_one() {
    let output = run(&["--sim", "--scenario", "require-close", "sale", "1000", "T1"]).unwrap();
    assert_code(&output, 1);
    assert!(stdout(&output).contains("74 Debe Ejecutar CIERRE"));
}

#[test]
fn communication_errors_exit_two() {
    let output = run(&["--sim", "--scenario", "drop-acks:3", "poll"]).unwrap();
    assert_code(&output, 2);
    assert!(
        stderr(&output).contains("did not acknowledge"),
        "{}",
        stderr(&output)
    );

    // Port 1 on loopback is closed: connection refused.
    let output = run(&["--tcp", "127.0.0.1:1", "poll"]).unwrap();
    assert_code(&output, 2);
}

#[test]
fn invalid_scenario_is_usage_error() {
    let output = run(&["--sim", "--scenario", "explode", "poll"]).unwrap();
    assert_code(&output, 64);
    assert!(stderr(&output).contains("escenario desconocido"));
}

#[test]
fn json_status_messages_go_to_stderr() {
    let output = run(&["--sim", "--json", "sale", "1000", "T1", "--status"]).unwrap();
    assert_code(&output, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(value["amount"], 1_000);
    assert!(stderr(&output).contains("… Esperando Leer Tarjeta"));
}

#[test]
fn json_lists_and_totals() {
    let output = run(&["--sim", "--json", "detail"]).unwrap();
    assert_code(&output, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    let entries = value.as_array().expect("array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["ticket"], "DEMO01");
    assert!(
        entries[0]["account_number"]
            .as_str()
            .unwrap_or_default()
            .starts_with("<redacted")
    );

    let output = run(&["--sim", "--json", "totals"]).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert_eq!(value["tx_count"], 1);
    assert_eq!(value["tx_total"], 12_000);
}

#[test]
fn json_show_voucher_keeps_the_voucher_only() {
    let output = run(&[
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
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert!(
        value["voucher"]
            .as_str()
            .unwrap_or_default()
            .starts_with("SIMULADOR 01")
    );
    assert!(
        value["account_number"]
            .as_str()
            .unwrap_or_default()
            .starts_with("<redacted")
    );
}

#[test]
fn ports_json_is_an_array() {
    let output = run(&["--json", "ports"]).unwrap();
    assert_code(&output, 0);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid JSON");
    assert!(value.is_array());
}

#[test]
fn raw_variants() {
    let output = run(&["--sim", "raw", "0100"]).unwrap();
    assert_code(&output, 0);
    assert_eq!(stdout(&output), "ACK recibido\n");

    let output = run(&["--sim", "raw", "0700||", "--expect", "0710"]).unwrap();
    assert_code(&output, 0);
    assert_eq!(stdout(&output), "<- 0710|00|001|12000||\n");

    // Node format of the sale (OPEN-01).
    let output = run(&["--sim", "raw", "0200|000000100|T00003|||0"]).unwrap();
    assert_code(&output, 0);
    assert!(stdout(&output).starts_with("<- 0210|00|"));

    assert_code(&run(&["--sim", "raw", "abc|"]).unwrap(), 64);
    assert_code(&run(&["--sim", "raw", ""]).unwrap(), 64);
    let output = run(&["--sim", "raw", "0250|", "--expect", "0810"]).unwrap();
    assert_code(&output, 2);
    assert!(stderr(&output).contains("expected response 0810"));
}

#[test]
fn ack_timeout_flag_is_applied() {
    // The simulator drops 3 ACKs: with the custom ACK timeout the error still
    // reports the 3 attempts of the default `max_attempts`.
    let output = run(&[
        "--sim",
        "--scenario",
        "drop-acks:3",
        "--ack-timeout-ms",
        "1",
        "poll",
    ])
    .unwrap();
    assert_code(&output, 2);
    assert!(stderr(&output).contains("after 3 attempts"));
}

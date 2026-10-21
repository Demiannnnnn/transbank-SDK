//! Smoke tests for the `tbk-pos` binary.

use std::process::Command;

#[test]
fn help_exits_zero_and_declares_unofficial() {
    let output = Command::new(env!("CARGO_BIN_EXE_tbk-pos"))
        .arg("--help")
        .output()
        .expect("run tbk-pos --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Unofficial"), "help output: {stdout}");
}

#[test]
fn version_matches_package() {
    let output = Command::new(env!("CARGO_BIN_EXE_tbk-pos"))
        .arg("--version")
        .output()
        .expect("run tbk-pos --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

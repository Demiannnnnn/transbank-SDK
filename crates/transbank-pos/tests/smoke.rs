//! Smoke tests for the crate metadata.

#[test]
fn license_is_dual_mit_apache() {
    assert_eq!(env!("CARGO_PKG_LICENSE"), "MIT OR Apache-2.0");
}

#[test]
fn description_declares_unofficial() {
    assert!(env!("CARGO_PKG_DESCRIPTION").starts_with("Unofficial"));
}

#[test]
fn dependency_and_profile_installation_drawbridge_is_explicit() {
    let runtime_manifest = include_str!("../Cargo.toml");
    let form_manifest = include_str!("../../conduit-form/Cargo.toml");
    let signal_manifest = include_str!("../../conduit-signal/Cargo.toml");
    let std_manifest = include_str!("../../../hosts/std/Cargo.toml");
    let std_host = include_str!("../../../hosts/std/src/lib.rs");
    let browser_package = include_str!("../../../package.json");
    let browser_config = include_str!("../../../hosts/browser/playwright.config.mjs");
    let justfile = include_str!("../../../justfile");

    assert!(!runtime_manifest.contains("conduit-signal"));
    assert!(!form_manifest.contains("conduit-signal"));
    assert!(signal_manifest.contains("default = []"));
    assert!(signal_manifest.contains("host-profile ="));
    assert!(std_manifest.contains("features = [\"host-profile\"]"));
    assert!(std_host.contains("signal_registry("));
    assert!(browser_package.contains("\"@playwright/test\": \"1.62.0\""));
    assert!(browser_config.contains("workers: 1"));
    assert!(browser_config.contains("retries: 0"));
    assert!(browser_config.contains("projects: [{ name: \"chromium\""));
    assert!(!browser_config.contains("firefox"));
    assert!(!browser_config.contains("webkit"));
    assert!(justfile.contains("check-browser-s4:"));
    assert!(justfile.contains("npm run test:browser-host"));
    assert!(justfile.contains("check-sim-readiness:"));
    assert!(justfile.contains("cargo test -p conduit-wire"));
    assert!(justfile.contains("cargo test -p conduit-browser-sim"));
    assert!(justfile.contains("cargo check -p conduit-browser-sim --target wasm32-unknown-unknown"));
    assert!(justfile.contains("cargo test -p conduit-pico-sim"));
    assert!(justfile.contains(
        "cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture"
    ));
    assert!(justfile.contains(
        "cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts"
    ));
    assert!(justfile.contains(
        "cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi"
    ));
    assert!(justfile.contains("--test host_contract"));
}

#[test]
fn readiness_contract_names_the_platform_stop_line() {
    let readiness = include_str!("../../../docs/architecture/browser-readiness.md");
    for required in [
        "contract/source -> contract/sink",
        "Controlled composite transport",
        "Parent-facing events",
        "cargo check -p conduit-signal --no-default-features",
        "conduit-wire",
        "fake browser-style adapter",
        "conduit-browser-sim",
        "multiple independent simulated browser instances",
        "wasm32-unknown-unknown",
        "bounded frame relay fixture using `conduit-wire`",
        "examples/triple-signal.form",
        "stdout, DOM-state, and onboard-LED-shaped receipts",
        "conduit-pico-sim",
        "thumbv6m-none-eabi",
        "onboard-LED receipts",
        "bounded datagram relay fixture using `conduit-wire`",
        "hosts/browser/signal-dom-host.mjs",
        "actual browser DOM effect/completion adapter",
        "Browser-side form execution",
        "physical Pico LED acceptance",
        "live WebSocket",
        "live UDP sockets",
    ] {
        assert!(
            readiness.contains(required),
            "missing readiness item: {required}"
        );
    }
}

#[test]
fn repository_status_matrix_keeps_proof_classes_distinct() {
    let status = include_str!("../../../STATUS.md");
    for required in [
        "Contract",
        "Simulation",
        "Executable hosted implementation",
        "Actual browser adapter",
        "Actual firmware",
        "Live transport",
        "Physical/HIL proof",
        "unsafe prototype disabled",
        "WASM compilation is not browser execution",
        "Thumb compilation is not firmware",
        "The Chromium DOM boundary is browser execution",
        "browser-side planner/runtime or a live link",
        "Frame/datagram fixtures",
        "WebSocket or UDP sockets",
    ] {
        assert!(
            status.contains(required),
            "missing status boundary: {required}"
        );
    }
}

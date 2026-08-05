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
    assert!(std_manifest.contains("legacy-fixture-driver = []"));
    let production_std = std_host
        .split_once("pub struct StdHost {")
        .and_then(|(_, remainder)| {
            remainder
                .split_once("pub struct LegacyStdFixtureHost")
                .map(|(production, _)| production)
        })
        .expect("production and legacy fixture std hosts remain distinct");
    assert!(!production_std.contains("HostRuntime"));
    assert!(!production_std.contains("HostCommand"));
    assert!(!production_std.contains("run_fragment_legacy"));
    assert!(std_host.contains("pub struct LegacyStdFixtureHost"));
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
fn production_browser_host_cannot_regain_the_legacy_runtime() {
    let manifest = include_str!("../../../hosts/browser-runtime/Cargo.toml");
    let browser = include_str!("../../../hosts/browser-runtime/src/lib.rs");
    let adapter = include_str!("../../../hosts/browser/signal-wasm-runtime.mjs");

    assert!(manifest.contains("conduit-kernel"));
    assert!(browser.contains("lower_plan_fragment"));
    assert!(browser.contains("type BrowserScheduler = FixedScheduler"));
    assert!(browser.contains("KernelExecutionIdentityMap"));
    assert!(!browser.contains("struct BrowserScheduler"));
    for forbidden in [
        "HostRuntime",
        "HostCommand",
        "HostEvent",
        "PlatformEffect",
        "signal_registry(",
    ] {
        assert!(
            !browser.contains(forbidden),
            "production browser host regained forbidden legacy symbol: {forbidden}"
        );
    }
    for forbidden in ["WebSocket", "Udp", "UDP", "Pico"] {
        assert!(
            !browser.contains(forbidden) && !adapter.contains(forbidden),
            "browser-local checkpoint crossed its transport/platform stop line: {forbidden}"
        );
    }
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
        "actual-browser timer/DOM effect adapter",
        "browser-local kernel implementation",
        "Physical Pico LED acceptance",
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
        "accepted Chromium proof is browser-local",
        "not a live link",
        "Frame/datagram fixtures",
        "WebSocket or UDP sockets",
    ] {
        assert!(
            status.contains(required),
            "missing status boundary: {required}"
        );
    }
}

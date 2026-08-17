#[test]
fn dependency_and_profile_installation_drawbridge_is_explicit() {
    let runtime_manifest = include_str!("../Cargo.toml");
    let form_manifest = include_str!("../../conduit-form/Cargo.toml");
    let signal_manifest = include_str!("../../conduit-signal/Cargo.toml");
    let std_manifest = include_str!("../../../hosts/std/Cargo.toml");
    let browser_manifest = include_str!("../../../hosts/browser-runtime/Cargo.toml");
    let firmware_manifest = include_str!("../../../firmware/conduit-pico-w-signal/Cargo.toml");
    let std_host = include_str!("../../../hosts/std/src/lib.rs");
    let browser_package = include_str!("../../../package.json");
    let browser_config = include_str!("../../../hosts/browser/playwright.config.mjs");
    let justfile = include_str!("../../../justfile");
    let check_suite = include_str!("../../../xtask/src/suites/check.rs");
    let prove_suite = include_str!("../../../xtask/src/suites/prove.rs");
    let test_corpus = format!("{justfile}\n{check_suite}\n{prove_suite}");

    assert!(!runtime_manifest.contains("conduit-signal"));
    assert!(runtime_manifest.contains("default = []"));
    assert!(runtime_manifest.contains("compatibility-executor = []"));
    assert!(!form_manifest.contains("conduit-signal"));
    assert!(signal_manifest.contains("default = []"));
    assert!(signal_manifest.contains("host-profile ="));
    assert!(signal_manifest.contains("legacy-fixture-driver ="));
    assert!(signal_manifest.contains("conduit-runtime/compatibility-executor"));
    assert!(std_manifest.contains("features = [\"host-profile\"]"));
    assert!(std_manifest.contains("legacy-fixture-driver = ["));
    for production_manifest in [std_manifest, browser_manifest, firmware_manifest] {
        assert!(
            production_manifest.contains(
                "conduit-runtime = { path = \"../../crates/conduit-runtime\", default-features = false }"
            ),
            "production dependency must expose lowering without compatibility execution"
        );
    }
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
    assert!(browser_config.contains("name: \"chromium\""));
    assert!(browser_config.contains("name: \"firefox\""));
    assert!(browser_config.contains("testMatch: [\"browser-webrtc-body.spec.mjs\"]"));
    assert!(!browser_config.contains("webkit"));
    assert!(justfile.contains("check-browser-s4:"));
    assert!(test_corpus.contains("test:browser-host"));
    assert!(justfile.contains("check-sim-readiness:"));
    assert!(test_corpus.contains("conduit-wire"));
    assert!(test_corpus.contains("conduit-browser-sim"));
    assert!(test_corpus.contains("wasm32-unknown-unknown"));
    assert!(test_corpus.contains("conduit-pico-sim"));
    assert!(test_corpus.contains("std_host_sends_signal_to_pico_through_bounded_datagram_fixture"));
    assert!(test_corpus.contains("triple_signal_form_fans_out_to_std_and_simulated_receipts"));
    assert!(test_corpus.contains("thumbv6m-none-eabi"));
    assert!(test_corpus.contains("host_contract"));
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
fn compatibility_executor_callers_are_explicitly_inventoried() {
    let inventory = include_str!("../../../docs/architecture/compatibility-execution-inventory.md");
    for required in [
        "`crates/conduit-runtime/src/compatibility_executor.rs`",
        "`hosts/std/src/lib.rs` / `LegacyStdFixtureHost`",
        "`fixtures/browser-sim`",
        "`fixtures/pico-sim`",
        "`crates/conduit-composite`",
        "`crates/conduit-signal/src/host_profile.rs`",
        "`crates/conduit-std-catalog/src/host_profile.rs`",
        "`crates/conduit-runtime/tests/host_contract.rs`",
    ] {
        assert!(
            inventory.contains(required),
            "missing caller inventory: {required}"
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
    let normalized_status = status.split_whitespace().collect::<Vec<_>>().join(" ");

    for required in [
        "| Surface | Contract | Simulation | Executable hosted implementation | Actual browser adapter | Actual firmware | Live transport | Physical/HIL proof |",
        "unsafe prototype disabled",
        "WASM compilation is not browser execution",
        "Thumb compilation proves",
        "not board execution or physical acceptance",
        "generated fixed image",
        "exact Pico-local, std↔Pico, and final std/browser/Pico board runs recorded",
        "attached-board Playwright cases remain explicitly hardware-gated",
        "fixture bases remain synthetic conformance only",
        "No UDP, Zenoh, TCP",
        "Frame/datagram fixtures are not WebSocket or UDP sockets",
    ] {
        assert!(
            normalized_status.contains(required),
            "missing status boundary: {required}"
        );
    }
}

use crate::process::Step;

pub static SIMULATION: &[Step] = &[
    Step::new(
        "sim-dep-check-signal",
        "Check conduit-runtime does not depend on conduit-signal (tree inspection handled separately)",
        "cargo",
        &["check", "-p", "conduit-signal", "--no-default-features"],
    ),
    Step::new(
        "sim-check-wire",
        "Check conduit-wire with no default features",
        "cargo",
        &["check", "-p", "conduit-wire", "--no-default-features"],
    ),
    Step::new(
        "sim-test-wire",
        "Test conduit-wire",
        "cargo",
        &["test", "-p", "conduit-wire"],
    ),
    Step::new(
        "sim-test-host-contract",
        "Test conduit-runtime host_contract",
        "cargo",
        &["test", "-p", "conduit-runtime", "--test", "host_contract"],
    ),
    Step::new(
        "sim-test-browser-sim",
        "Test conduit-browser-sim",
        "cargo",
        &["test", "-p", "conduit-browser-sim"],
    ),
    Step::new(
        "sim-test-triple-signal",
        "Test triple_signal_form_fans_out_to_std_and_simulated_receipts",
        "cargo",
        &["test", "-p", "conduit-browser-sim", "triple_signal_form_fans_out_to_std_and_simulated_receipts"],
    ),
    Step::new(
        "sim-check-browser-sim-wasm",
        "Check conduit-browser-sim for wasm32-unknown-unknown",
        "cargo",
        &["check", "-p", "conduit-browser-sim", "--target", "wasm32-unknown-unknown"],
    ),
    Step::new(
        "sim-test-pico-sim",
        "Test conduit-pico-sim",
        "cargo",
        &["test", "-p", "conduit-pico-sim"],
    ),
    Step::new(
        "sim-test-pico-datagram",
        "Test std_host_sends_signal_to_pico_through_bounded_datagram_fixture",
        "cargo",
        &["test", "-p", "conduit-pico-sim", "std_host_sends_signal_to_pico_through_bounded_datagram_fixture"],
    ),
    Step::new(
        "sim-check-pico-sim-thumb",
        "Check conduit-pico-sim for thumbv6m-none-eabi (no default features)",
        "cargo",
        &["check", "-p", "conduit-pico-sim", "--no-default-features", "--target", "thumbv6m-none-eabi"],
    ),
];

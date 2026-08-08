use crate::{process::Step, proof::ProofClass};

pub const PROVE_STD_BROWSER_S4_STEPS: &[Step] = &[
    Step::typed(
        "prove.std-browser-s4.wasm-build",
        "Build conduit-browser-runtime WASM artifact",
        "cargo",
        &[
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
        None,
        Some("wasm32-unknown-unknown"),
        Some(ProofClass::ContractCompile),
        &["hosts/browser/conduit_browser_runtime.wasm"],
    ),
    Step::typed(
        "prove.std-browser-s4.playwright",
        "Run Playwright distributed signal spec",
        "npx",
        &[
            "playwright",
            "test",
            "--config",
            "hosts/browser/playwright.config.mjs",
            "hosts/browser/distributed-signal.spec.mjs",
        ],
        None,
        Some("playwright"),
        Some(ProofClass::LiveBrowser),
        &[],
    ),
];

pub const PROVE_STD_BROWSER_TOGGLE_STEPS: &[Step] = &[
    Step::typed(
        "prove.std-browser-toggle.wasm-build",
        "Build conduit-browser-runtime WASM artifact",
        "cargo",
        &[
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
        None,
        Some("wasm32-unknown-unknown"),
        Some(ProofClass::ContractCompile),
        &["hosts/browser/conduit_browser_runtime.wasm"],
    ),
    Step::typed(
        "prove.std-browser-toggle.playwright",
        "Run Playwright distributed toggle spec",
        "npx",
        &[
            "playwright",
            "test",
            "--config",
            "hosts/browser/playwright.config.mjs",
            "hosts/browser/distributed-toggle.spec.mjs",
        ],
        None,
        Some("playwright"),
        Some(ProofClass::LiveBrowser),
        &[],
    ),
];

pub const PROVE_BROWSER_HOST_STEPS: &[Step] = &[
    Step::typed(
        "prove.browser-host.wasm-build",
        "Build conduit-browser-runtime WASM artifact",
        "cargo",
        &[
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
        None,
        Some("wasm32-unknown-unknown"),
        Some(ProofClass::ContractCompile),
        &["hosts/browser/conduit_browser_runtime.wasm"],
    ),
    Step::typed(
        "prove.browser-host.webchat-server-build",
        "Build bounded WebSocket chat server artifact",
        "cargo",
        &["build", "-p", "conduit-std-host", "--bin", "webchat-server"],
        None,
        None,
        Some(ProofClass::ContractCompile),
        &["target/debug/webchat-server"],
    ),
    Step::typed(
        "prove.browser-host.playwright",
        "Run browser host test suite",
        "npm",
        &["run", "test:browser-host"],
        None,
        Some("playwright"),
        Some(ProofClass::LiveBrowser),
        &[],
    ),
];

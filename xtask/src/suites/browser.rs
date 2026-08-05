use crate::process::Step;

pub static BROWSER_S4: &[Step] = &[
    Step::new(
        "browser-s4-test-runtime",
        "Test conduit-browser-runtime",
        "cargo",
        &["test", "-p", "conduit-browser-runtime"],
    ),
    Step::new(
        "browser-s4-no-legacy-runtime",
        "Test production_browser_host_cannot_regain_the_legacy_runtime",
        "cargo",
        &[
            "test", "-p", "conduit-runtime", "--test", "browser_readiness",
            "production_browser_host_cannot_regain_the_legacy_runtime",
        ],
    ),
    Step::new(
        "browser-s4-wasm-build",
        "Build conduit-browser-runtime for wasm32-unknown-unknown",
        "cargo",
        &["build", "-p", "conduit-browser-runtime", "--target", "wasm32-unknown-unknown", "--release"],
    ),
    Step::new(
        "browser-s4-npm-test",
        "Run browser host npm tests",
        "npm",
        &["run", "test:browser-host"],
    ),
];

/// Steps for the `prove std-browser-s4` command.
pub static PROVE_STD_BROWSER_S4: &[Step] = &[
    Step::new(
        "prove-std-browser-wasm-build",
        "Build conduit-browser-runtime for wasm32-unknown-unknown (release)",
        "cargo",
        &["build", "-p", "conduit-browser-runtime", "--target", "wasm32-unknown-unknown", "--release"],
    ),
    Step::new(
        "prove-std-browser-playwright",
        "Run Playwright distributed-signal spec",
        "npx",
        &[
            "playwright",
            "test",
            "--config",
            "hosts/browser/playwright.config.mjs",
            "hosts/browser/distributed-signal.spec.mjs",
        ],
    ),
];

use crate::{process::Step, proof::ProofClass};

pub const BROWSER_CHECK_STEPS: &[Step] = &[
    Step::new(
        "check.browser.browser-runtime",
        "test conduit-browser-runtime",
        "cargo",
        &["test", "-p", "conduit-browser-runtime"],
    ),
    Step::new(
        "check.browser.host-calls",
        "Prove finite generic browser Host Calls and negative outcomes",
        "node",
        &["--test", "proof/browser/browser-host-calls.test.mjs"],
    ),
    Step::new(
        "check.browser.creche-rendezvous",
        "Prove bounded one-use Crèche running-Host rendezvous codes",
        "node",
        &["--test", "proof/browser/creche-rendezvous.test.mjs"],
    ),
    Step::new(
        "check.browser.physical-host-workflow",
        "Prove deterministic physical Host catalog and workflow contracts",
        "node",
        &["--test", "proof/browser/physical-host-workflow.test.mjs"],
    ),
    Step::new(
        "check.browser.sdk-contracts",
        "Prove the public Browser SDK, events, Forms, and exact package closure",
        "node",
        &[
            "--test",
            "targets/browser/sdk/browser-sdk-errors.test.mjs",
            "targets/browser/sdk/browser-sdk-events.test.mjs",
            "targets/browser/sdk/browser-sdk-forms.test.mjs",
            "targets/browser/sdk/package-browser-bundle.test.mjs",
        ],
    ),
    Step::typed(
        "check.browser.wasm-build",
        "Build conduit-browser-runtime WASM",
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
        &[],
    ),
    Step::new(
        "check.browser.host-entrance-build",
        "Build the standalone browser Host entrance",
        "cargo",
        &["build", "-p", "conduit-browser-host"],
    ),
];

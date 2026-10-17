use crate::{process::Step, proof::ProofClass};

pub const BROWSER_CHECK_STEPS: &[Step] = &[
    Step::new(
        "check.browser.shared-face-style-package",
        "Check shared Face stylesheet bounds across every application package consumer",
        "node",
        &["--test", "proof/browser/shared-face-style-package.test.mjs"],
    ),
    Step::new(
        "check.browser.saved-choice",
        "Check finite saved-choice restore, committed emission, and distinct storage refusals",
        "node",
        &["--test", "proof/browser/browser-saved-choice.test.mjs"],
    ),
    Step::new(
        "check.browser.application-syntax-face",
        "Check bounded lossless browser syntax projection admission",
        "node",
        &["--test", "proof/browser/application-syntax-face.test.mjs"],
    ),
    Step::new(
        "check.browser.application-face-codec",
        "Check exact Unicode and identity fidelity in the generic application Face codec",
        "node",
        &["--test", "proof/browser/application-face-codec.test.mjs"],
    ),
    Step::new(
        "check.browser.runtime-bridge",
        "Check finite exact-text ingress and byte custody at the browser runtime ABI",
        "node",
        &["--test", "targets/browser/host/assets/browser-runtime-bridge.test.mjs"],
    ),
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
        "check.browser.look-wire",
        "Check canonical Look wire identities, tokens, and finite byte admission",
        "node",
        &["--test", "proof/browser/look-wire.test.mjs"],
    ),
    Step::new(
        "check.browser.look-replacement-evidence",
        "Check Look replacement evidence comparisons and mutation refusals",
        "node",
        &["--test", "proof/browser/look-replacement-evidence.test.mjs"],
    ),
    Step::new(
        "check.browser.sdk-contracts",
        "Prove the public Browser SDK, events, Plots, and exact package closure",
        "node",
        &[
            "--test",
            "targets/browser/sdk/browser-sdk-errors.test.mjs",
            "targets/browser/sdk/browser-sdk-events.test.mjs",
            "targets/browser/sdk/browser-sdk-plots.test.mjs",
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
    )
    .with_environment(&crate::process::BROWSER_RELEASE_ENVIRONMENT),
    Step::new(
        "check.browser.host-entrance-build",
        "Build the standalone browser Host entrance",
        "cargo",
        &["build", "-p", "conduit-browser-host"],
    ),
];

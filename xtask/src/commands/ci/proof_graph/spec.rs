use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ProofKind {
    Workspace,
    Browser,
    Machine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Applicability {
    CandidateAndIntegration,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ProofSpec {
    pub(super) id: &'static str,
    pub(super) contract_version: u32,
    pub(super) kind: ProofKind,
    pub(super) inputs: &'static [&'static str],
    pub(super) implementation_inputs: &'static [&'static str],
    pub(super) consumed_artifacts: &'static [&'static str],
    pub(super) environment: &'static str,
    pub(super) applicability: Applicability,
    pub(super) command: &'static str,
}

// This first registry slice names broad, reviewable domains. Missing knowledge
// fails closed by selecting the broader domain; later slices can split a node
// without changing the proof-key or receipt contract.
pub(super) const PROOFS: &[ProofSpec] = &[
    ProofSpec {
        id: "workspace.products",
        contract_version: 2,
        kind: ProofKind::Workspace,
        inputs: &[
            "Cargo.toml",
            "Cargo.lock",
            "architecture",
            "fabrication",
            "semantics",
            "products",
            "bodies",
            "targets/std",
        ],
        implementation_inputs: &[
            "xtask/src/commands/check.rs",
            "xtask/src/suites",
            ".github/workflows/check.yml",
        ],
        consumed_artifacts: &[],
        environment: "ubuntu-stable-rust-v1",
        applicability: Applicability::CandidateAndIntegration,
        command: "cargo xtask check workspace-test-products --locked",
    },
    ProofSpec {
        id: "browser.tour",
        contract_version: 2,
        kind: ProofKind::Browser,
        inputs: &[
            "Cargo.toml",
            "Cargo.lock",
            "architecture/form",
            "architecture/kernel",
            "targets/browser",
            "products/patchbay/html",
            "products/tour",
            "site",
        ],
        implementation_inputs: &[
            "proof/browser/executable-book.spec.mjs",
            "proof/browser/browser-application-package.spec.mjs",
            "proof/browser/playwright.config.mjs",
            "scripts/ci/stage-book-product.sh",
            ".github/workflows/book-pr-proof.yml",
        ],
        // Runtime-byte promotion is a later artifact node; this proof currently
        // fingerprints the runtime sources and fabrication contract directly.
        consumed_artifacts: &[],
        environment: "playwright-chromium-1.62.0-noble-worker1-retry0",
        applicability: Applicability::CandidateAndIntegration,
        command: "npx playwright test --config proof/browser/playwright.config.mjs proof/browser/executable-book.spec.mjs --project chromium --workers 1 --retries 0",
    },
    ProofSpec {
        id: "machine.esp32-c3",
        contract_version: 1,
        kind: ProofKind::Machine,
        inputs: &[
            "Cargo.toml",
            "Cargo.lock",
            "architecture",
            "fabrication",
            "semantics",
            "targets/esp32",
        ],
        implementation_inputs: &[
            "xtask/src/commands/esp32_firmware.rs",
            ".github/workflows/check.yml",
        ],
        consumed_artifacts: &[],
        environment: "ubuntu-rust-1.91.1-riscv32imc-v1",
        applicability: Applicability::CandidateAndIntegration,
        command: "cargo xtask esp32-firmware build --target c3 --locked",
    },
];

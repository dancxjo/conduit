use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::suites::workspace_shards::WorkspaceShard;
use crate::suites::{
    check::WORKSPACE_STEPS, network_capability::NETWORK_CAPABILITY_STEPS,
    pico_compositions::PICO_COMPOSITION_STEPS,
};

#[path = "impact/cargo_graph.rs"]
mod cargo_graph;
#[path = "impact/command_registry.rs"]
mod command_registry;
#[path = "impact/git_changes.rs"]
mod git_changes;
#[path = "impact/product_registry.rs"]
pub(super) mod product_registry;
use cargo_graph::{affected_tests, dependency_closure, discover, normalize, Package};
use command_registry::{proofs_for_path, HeavySuite};
use git_changes::candidate_changed_paths;
use product_registry::{
    browser_presentation_proofs_for_path, proofs_for_paths as product_proofs_for_paths,
};

const SUITES: [&str; 3] = ["esp32", "browser", "conduitos"];
const ESP32_TARGETS: [&str; 3] = ["wroom", "c3", "s3"];
const CONDUITOS_X86_PROOFS: [&str; 8] = [
    "kernel",
    "xhci",
    "usb",
    "hid",
    "keyboard",
    "front-door",
    "product-journey",
    "rescue",
];
const CONDUITOS_ARCHITECTURES: [&str; 4] = ["aarch64", "ia32", "riscv64", "loongarch64"];
const GLOBAL_PREFIXES: [&str; 5] = [
    ".github/",
    ".cargo/",
    "xtask/",
    "scripts/ci/",
    "tools/xtask-dispatch/",
];
const GLOBAL_FILES: [&str; 3] = ["Cargo.toml", "rust-toolchain", "rust-toolchain.toml"];
const FOCUSED_WORKFLOW_FILES: [&str; 1] = [".github/workflows/executable-book-pages.yml"];

struct ControllerProofSpec {
    id: &'static str,
    implementation_inputs: &'static [&'static str],
    required_inputs: &'static [&'static str],
    workspace_packages: &'static [&'static str],
}

const CONTROLLER_PROOFS: &[ControllerProofSpec] = &[
    ControllerProofSpec {
        id: "ci.candidate-retirement",
        implementation_inputs: &[
            ".github/workflows/retire-superseded-candidates.yml",
            "proof/ci/retire-superseded-candidates.spec.mjs",
            "scripts/ci/retire-superseded-candidates.mjs",
        ],
        required_inputs: &[],
        workspace_packages: &[],
    },
    ControllerProofSpec {
        id: "ci.current-controller-reconciliation",
        implementation_inputs: &[
            ".github/workflows/reconcile-candidate.yml",
            "proof/ci/reconcile-candidate-request.spec.mjs",
            "scripts/ci/reconcile-candidate-request.mjs",
        ],
        required_inputs: &[],
        workspace_packages: &[],
    },
    ControllerProofSpec {
        id: "ci.exact-integration",
        implementation_inputs: &[
            ".github/workflows/reconcile-candidate.yml",
            "tools/xtask-dispatch/src/ci_dispatch.rs",
            "tools/xtask-dispatch/src/main.rs",
            "xtask/src/commands/ci.rs",
            "xtask/src/commands/ci/integration.rs",
            "xtask/src/commands/ci/integration/tests.rs",
            "xtask/src/commands/ci/impact.rs",
            "xtask/src/commands/ci/impact/tests.rs",
            "xtask/src/commands/ci/proof_graph.rs",
            "xtask/tests/ci_workflow_contract.rs",
        ],
        // Shared dispatch and proof-planning files are bounded only when the
        // exact integration resolver anchors the change.
        required_inputs: &[
            "xtask/src/commands/ci/integration.rs",
            "xtask/src/commands/ci/integration/tests.rs",
        ],
        workspace_packages: &["conduit-xtask-dispatch", "xtask"],
    },
    ControllerProofSpec {
        id: "ci.planner-contract-tests",
        implementation_inputs: &[
            ".github/workflows/check.yml",
            "tools/xtask-dispatch/Cargo.toml",
            "tools/xtask-dispatch/src/main.rs",
            "xtask/src/commands/ci/impact.rs",
            "xtask/src/commands/ci/impact/tests.rs",
            "xtask/tests/ci_workflow_contract.rs",
        ],
        // A workflow or planner edit alone remains conservatively broad. The
        // test-target manifest is the dependency-light slice anchor.
        required_inputs: &["tools/xtask-dispatch/Cargo.toml"],
        workspace_packages: &["conduit-xtask-dispatch", "xtask"],
    },
    ControllerProofSpec {
        id: "ci.merged-branch-retirement",
        implementation_inputs: &[
            ".github/workflows/retire-merged-pr-branch.yml",
            "proof/ci/retire-merged-pr-branch.spec.mjs",
            "scripts/ci/retire-merged-pr-branch.mjs",
            "xtask/src/commands/ci/impact.rs",
            "xtask/src/commands/ci/impact/tests.rs",
            "xtask/tests/ci_workflow_contract.rs",
        ],
        // Planner/test files are bounded to this controller only when its
        // complete executable slice anchors the same change.
        required_inputs: &[
            ".github/workflows/retire-merged-pr-branch.yml",
            "proof/ci/retire-merged-pr-branch.spec.mjs",
            "scripts/ci/retire-merged-pr-branch.mjs",
        ],
        workspace_packages: &["xtask"],
    },
    ControllerProofSpec {
        id: "ci.actions-monitor",
        implementation_inputs: &[
            "tools/xtask-dispatch/src/main.rs",
            "tools/xtask-dispatch/src/ci_dispatch.rs",
            "xtask/src/commands/ci.rs",
            "xtask/src/commands/ci/monitor.rs",
        ],
        // Shared dispatcher files are controller-local only when the isolated
        // monitor implementation anchors the change. Alone they remain in the
        // conservative fallback because they route other CI commands.
        required_inputs: &["xtask/src/commands/ci/monitor.rs"],
        workspace_packages: &["conduit-xtask-dispatch", "xtask"],
    },
    ControllerProofSpec {
        id: "ci.check-result-gate",
        implementation_inputs: &[
            ".github/workflows/check.yml",
            "proof/ci/check-result-gate.spec.mjs",
            "xtask/src/commands/ci/impact.rs",
            "xtask/src/commands/ci/impact/tests.rs",
            "xtask/tests/ci_workflow_contract.rs",
        ],
        // The workflow and its executable truth table anchor this bounded
        // slice. An arbitrary planner or workflow-test edit remains global.
        required_inputs: &[
            ".github/workflows/check.yml",
            "proof/ci/check-result-gate.spec.mjs",
        ],
        workspace_packages: &["xtask"],
    },
];

fn active_controller_proofs(paths: &[String]) -> Vec<&'static ControllerProofSpec> {
    CONTROLLER_PROOFS
        .iter()
        .filter(|spec| {
            paths
                .iter()
                .any(|path| spec.implementation_inputs.contains(&path.as_str()))
                && spec
                    .required_inputs
                    .iter()
                    .all(|required| paths.iter().any(|path| path == required))
        })
        .collect()
}

fn controller_proofs(paths: &[String]) -> Vec<&'static str> {
    active_controller_proofs(paths)
        .into_iter()
        .map(|spec| spec.id)
        .collect()
}
const PAGES_DEPLOY_RESOLVER_SLICE: [&str; 9] = [
    ".github/workflows/executable-book-pages.yml",
    ".github/workflows/executable-book-deploy.yml",
    ".github/workflows/pages-deploy-pr-proof.yml",
    "proof/ci/pages-product-run-selection.spec.mjs",
    "proof/ci/pages-workflow-paths.spec.mjs",
    "scripts/ci/pages-product-run-selection.mjs",
    "scripts/ci/resolve-pages-product-run.mjs",
    "xtask/src/commands/ci/impact.rs",
    "xtask/src/commands/ci/impact/tests.rs",
];
const HARMLESS_PREFIXES: [&str; 1] = ["docs/"];
const HARMLESS_FILES: [&str; 6] = [
    "README.md",
    "STATUS.md",
    "AGENTS.md",
    "LICENSE",
    "justfile",
    "Justfile",
];
const DEBUGGER_KERNEL_SLICE: [&str; 7] = [
    "architecture/kernel/src/debug_observation.rs",
    "architecture/kernel/src/debug_observation/buffer.rs",
    "architecture/kernel/src/debug_observation/control.rs",
    "architecture/kernel/src/debug_observation/sink.rs",
    "architecture/kernel/src/scheduler.rs",
    "architecture/kernel/src/scheduler/debug_control.rs",
    "architecture/kernel/tests/debug_observation.rs",
];
const PATCHBAY_PACKAGE_SLICE: [&str; 11] = [
    "Cargo.lock",
    "products/patchbay/html/Cargo.toml",
    "products/patchbay/html/assets/app.css",
    "products/patchbay/html/assets/app.js",
    "products/patchbay/html/assets/index.html",
    "products/patchbay/html/assets/patchbay.application.template.json",
    "products/patchbay/html/src/server.rs",
    "products/patchbay/html/src/server/http.rs",
    "products/patchbay/html/tests/server.rs",
    "proof/browser/patchbay-debugger-watch.spec.mjs",
    "proof/browser/patchbay-html.spec.mjs",
];
const PI_ZERO_CRECHE_SLICE: [&str; 12] = [
    ".github/workflows/executable-book-pages.yml",
    "fabrication/workspace/tests/family_contracts.rs",
    "proof/browser/executable-book.spec.mjs",
    "scripts/ci/stage-creche-product.sh",
    "targets/browser/runtime/src/creche/spore_target.rs",
    "targets/raspberry-pi/browser-deployment/creche-adapter.mjs",
    "targets/raspberry-pi/browser-deployment/image.mjs",
    "targets/raspberry-pi/fabrication-package/src/lib.rs",
    "targets/raspberry-pi/fabrication/xtask/armv6_rpi_b_plus_image.rs",
    "targets/raspberry-pi/fabrication/xtask/armv6_rpi_board.rs",
    "targets/std/browser-deployment/creche-adapter.mjs",
    "xtask/src/commands/host_release.rs",
];
fn is_tongues_analysis_path(path: &str) -> bool {
    path.starts_with("semantics/tongues/")
        || path == "forms/tongues-dynamics-analysis/main.conduit"
        || path == "products/patchbay/html/src/learned_demo.rs"
        || path == "proof/browser/patchbay-debugger-watch.spec.mjs"
        || path == "xtask/src/commands/ci/impact.rs"
        || path == "xtask/src/commands/ci/impact/tests.rs"
        || matches!(
            path,
            "xtask/src/cli.rs" | "xtask/src/main.rs" | "xtask/src/commands/tongues.rs"
        )
}

fn is_creche_presentation_path(path: &str) -> bool {
    path == "proof/browser/executable-book.spec.mjs"
        || path == "scripts/ci/stage-creche-product.sh"
        || path == "targets/browser/host/src/server.rs"
        || path == "targets/browser/host/src/server/tests.rs"
        || path == "targets/browser/host/assets/application-presentation.mjs"
        || path.starts_with("products/creche/browser/creche")
}

fn is_repository_tool_test(path: &str) -> bool {
    path.starts_with("xtask/tests/") && path.ends_with(".rs")
}

fn machine_proof_is_required_for_dependency(path: &str, suite: &str) -> bool {
    // Semantic crates are renderer- and machine-neutral contracts. Their
    // reverse-dependent workspace shards compile and test the affected product
    // graph, including portable/embedded configurations. Fabricating firmware
    // and booting every machine adds no distinct proof unless the change also
    // touches a target-sensitive layer, which is classified separately.
    !path.starts_with("semantics/") || !matches!(suite, "esp32" | "conduitos")
}

#[derive(Debug, Serialize)]
struct ImpactPlan {
    requested_base_sha: Option<String>,
    candidate_sha: Option<String>,
    candidate_comparison_base_sha: Option<String>,
    ci_controller_proofs: Vec<&'static str>,
    repository_command_proofs: Vec<&'static str>,
    pages_product_proofs: Vec<&'static str>,
    pages_products_required: bool,
    esp32_required: bool,
    esp32_targets: Vec<String>,
    browser_required: bool,
    conduitos_required: bool,
    conduitos_x86_proofs: Vec<String>,
    conduitos_architectures: Vec<String>,
    conduitos_aarch64_product_required: bool,
    full_fallback: bool,
    reason: String,
    changed_paths: Vec<String>,
    changed_packages: Vec<String>,
    affected_test_packages: Vec<String>,
    shared_compile_packages: Vec<String>,
    workspace_lint_full: bool,
    workspace_lint_packages: Vec<String>,
    workspace_shards: BTreeMap<String, bool>,
    suite_reasons: BTreeMap<String, Vec<String>>,
}

struct WorkspaceImpact {
    changed_packages: BTreeSet<String>,
    affected_test_packages: BTreeSet<String>,
    lint_packages: BTreeSet<String>,
    shards: BTreeMap<String, bool>,
}

struct MachineImpact {
    esp32: Esp32Impact,
    conduitos: ConduitosImpact,
}

#[derive(Default)]
struct Esp32Impact {
    targets: BTreeSet<String>,
}

impl Esp32Impact {
    fn all() -> Self {
        Self {
            targets: ESP32_TARGETS.map(str::to_owned).into_iter().collect(),
        }
    }

    fn required(&self) -> bool {
        !self.targets.is_empty()
    }
}

#[derive(Default)]
struct ConduitosImpact {
    x86_proofs: BTreeSet<String>,
    architectures: BTreeSet<String>,
    aarch64_product: bool,
}

impl ConduitosImpact {
    fn all() -> Self {
        Self {
            x86_proofs: CONDUITOS_X86_PROOFS
                .map(str::to_owned)
                .into_iter()
                .collect(),
            architectures: CONDUITOS_ARCHITECTURES
                .map(str::to_owned)
                .into_iter()
                .collect(),
            aarch64_product: true,
        }
    }

    fn required(&self) -> bool {
        !self.x86_proofs.is_empty() || !self.architectures.is_empty() || self.aarch64_product
    }
}

pub(super) fn run(
    base: &str,
    head: &str,
    json_out: Option<&Path>,
    summary_out: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let mut comparison_base = None;
    let mut extracted_paths = None;
    let mut plan = match candidate_changed_paths(&root, base, head)
        .and_then(|change_set| {
            comparison_base = Some(change_set.comparison_base);
            let planning_paths = if change_set.test_extraction_parents.is_empty() {
                change_set.paths
            } else {
                extracted_paths = Some(change_set.paths);
                change_set.test_extraction_parents
            };
            discover(&root).map(|packages| (planning_paths, packages))
        })
        .and_then(|(paths, packages)| plan_for_paths(&root, paths, &packages))
    {
        Ok(plan) => plan,
        Err(error) => full_plan(format!("planner-error:{error}"), Vec::new()),
    };
    plan.requested_base_sha = Some(base.to_owned());
    plan.candidate_sha = Some(head.to_owned());
    plan.candidate_comparison_base_sha = comparison_base;
    if let Some(paths) = extracted_paths {
        retain_only_workspace_proofs_for_test_extraction(&mut plan, paths);
    }

    write_github_outputs(&plan);
    if let Some(path) = json_out {
        write_parent(path)?;
        fs::write(path, format!("{}\n", serde_json::to_string_pretty(&plan)?))?;
    }
    if let Some(path) = summary_out {
        write_parent(path)?;
        fs::write(path, markdown_summary(&plan))?;
    }
    Ok(())
}

fn retain_only_workspace_proofs_for_test_extraction(plan: &mut ImpactPlan, paths: Vec<String>) {
    plan.ci_controller_proofs.clear();
    plan.repository_command_proofs.clear();
    plan.pages_product_proofs.clear();
    plan.pages_products_required = false;
    plan.esp32_required = false;
    plan.esp32_targets.clear();
    plan.browser_required = false;
    plan.conduitos_required = false;
    plan.conduitos_x86_proofs.clear();
    plan.conduitos_architectures.clear();
    plan.conduitos_aarch64_product_required = false;
    plan.full_fallback = false;
    plan.reason = "behavior-preserving-test-extraction".to_owned();
    plan.changed_paths = paths;
    plan.suite_reasons = empty_reasons();
}

fn suite_roots() -> BTreeMap<&'static str, BTreeSet<&'static str>> {
    BTreeMap::from([
        (
            "esp32",
            BTreeSet::from([
                "conduit-esp32-c3-signal",
                "conduit-esp32-s3-signal",
                "conduit-esp32-wroom-signal",
                "conduit-host-esp32-fabrication",
            ]),
        ),
        (
            "browser",
            BTreeSet::from([
                "conduit-browser-host",
                "conduit-browser-runtime",
                "patchbay-html",
                "patchbay-hosted",
                "patchbay-control",
                "patchbay-model",
                "patchbay-native",
            ]),
        ),
        (
            "conduitos",
            BTreeSet::from([
                "conduitos",
                "conduit-host-conduitos-fabrication",
                "conduit-workspace-fabrication",
            ]),
        ),
    ])
}

fn direct_prefixes(suite: &str) -> &'static [&'static str] {
    match suite {
        "esp32" => &["targets/esp32/"],
        "browser" => &[
            "targets/browser/",
            "products/patchbay/",
            "proof/browser/",
            "assets/",
        ],
        "conduitos" => &["targets/conduitos/", "profiles/hosts/conduitos"],
        _ => &[],
    }
}

fn plan_for_paths(
    root: &Path,
    paths: Vec<String>,
    packages: &BTreeMap<String, Package>,
) -> Result<ImpactPlan, String> {
    let closures: BTreeMap<_, _> = suite_roots()
        .iter()
        .map(|(suite, roots)| {
            dependency_closure(packages, roots).map(|closure| ((*suite).to_owned(), closure))
        })
        .collect::<Result<_, _>>()?;
    let mut selected = BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), false)));
    let mut esp32 = Esp32Impact::default();
    let mut conduitos = ConduitosImpact::default();
    let mut reasons = empty_reasons();
    let mut changed_packages = BTreeSet::new();
    let mut repository_command_proofs = BTreeSet::new();
    let mut check_workflow_changed = false;
    let substantive: Vec<_> = paths.iter().filter(|path| !path.ends_with(".md")).collect();
    if substantive.is_empty() {
        return Ok(plan(
            selected,
            MachineImpact { esp32, conduitos },
            false,
            "markdown-only".to_owned(),
            paths,
            WorkspaceImpact {
                changed_packages,
                affected_test_packages: BTreeSet::new(),
                lint_packages: BTreeSet::new(),
                shards: WorkspaceShard::ALL
                    .into_iter()
                    .map(|shard| (shard.name().to_owned(), false))
                    .collect(),
            },
            reasons,
        ));
    }
    let active_controller_proofs = active_controller_proofs(&paths);
    let controller_only = substantive.iter().all(|path| {
        active_controller_proofs
            .iter()
            .any(|spec| spec.implementation_inputs.contains(&path.as_str()))
    });

    // The scheduler façade is normally a whole-platform dependency. Admit the
    // narrower classification only for the complete, recognizable debugger
    // control slice; a scheduler.rs change by itself still selects every
    // dependent platform.
    let debugger_kernel_slice = substantive
        .iter()
        .filter(|path| path.starts_with("architecture/kernel/"))
        .all(|path| DEBUGGER_KERNEL_SLICE.contains(&path.as_str()))
        && substantive
            .iter()
            .any(|path| path.as_str() == "architecture/kernel/src/debug_observation/control.rs")
        && substantive
            .iter()
            .any(|path| path.as_str() == "architecture/kernel/src/scheduler/debug_control.rs")
        && substantive
            .iter()
            .any(|path| path.as_str() == "architecture/kernel/src/scheduler.rs");
    let patchbay_package_slice = substantive
        .iter()
        .all(|path| PATCHBAY_PACKAGE_SLICE.contains(&path.as_str()))
        && [
            "Cargo.lock",
            "products/patchbay/html/Cargo.toml",
            "products/patchbay/html/assets/patchbay.application.template.json",
            "products/patchbay/html/assets/index.html",
            "products/patchbay/html/src/server.rs",
        ]
        .iter()
        .all(|required| substantive.iter().any(|path| path.as_str() == *required));
    let pi_zero_creche_slice = substantive
        .iter()
        .all(|path| PI_ZERO_CRECHE_SLICE.contains(&path.as_str()))
        && [
            ".github/workflows/executable-book-pages.yml",
            "proof/browser/executable-book.spec.mjs",
            "scripts/ci/stage-creche-product.sh",
            "targets/browser/runtime/src/creche/spore_target.rs",
            "targets/raspberry-pi/fabrication-package/src/lib.rs",
        ]
        .iter()
        .all(|required| substantive.iter().any(|path| path.as_str() == *required));
    let creche_presentation_slice = substantive
        .iter()
        .all(|path| is_creche_presentation_path(path))
        && substantive
            .iter()
            .any(|path| path.starts_with("products/creche/browser/creche"))
        && substantive
            .iter()
            .any(|path| path.as_str() == "proof/browser/executable-book.spec.mjs");
    let pages_deploy_resolver_slice = substantive
        .iter()
        .all(|path| PAGES_DEPLOY_RESOLVER_SLICE.contains(&path.as_str()))
        && substantive
            .iter()
            .any(|path| path.as_str() == "scripts/ci/resolve-pages-product-run.mjs")
        && substantive
            .iter()
            .any(|path| path.as_str() == "proof/ci/pages-product-run-selection.spec.mjs");
    let tongues_analysis_slice = substantive
        .iter()
        .all(|path| is_tongues_analysis_path(path))
        && substantive
            .iter()
            .any(|path| path.starts_with("semantics/tongues/"))
        && substantive
            .iter()
            .any(|path| path.as_str() == "products/patchbay/html/src/learned_demo.rs")
        && substantive
            .iter()
            .any(|path| path.as_str() == "proof/browser/patchbay-debugger-watch.spec.mjs")
        && substantive
            .iter()
            .any(|path| path.as_str() == "xtask/src/commands/tongues.rs");
    // A workspace lock update caused by an accompanying package manifest is
    // covered by package dependency closure. A lock-only change remains a
    // global fallback because no bounded ownership explains it.
    let manifest_backed_lock = substantive.iter().any(|path| path.as_str() == "Cargo.lock")
        && substantive
            .iter()
            .any(|path| path.as_str() != "Cargo.toml" && path.ends_with("/Cargo.toml"));
    for path in substantive {
        let owning_controller_proofs = active_controller_proofs
            .iter()
            .filter(|spec| spec.implementation_inputs.contains(&path.as_str()))
            .copied()
            .collect::<Vec<_>>();
        if !owning_controller_proofs.is_empty() {
            changed_packages.extend(owning_controller_proofs.iter().flat_map(|spec| {
                spec.workspace_packages
                    .iter()
                    .map(|package| (*package).to_owned())
            }));
            continue;
        }
        if pages_deploy_resolver_slice {
            continue;
        }
        let command_proofs = proofs_for_path(path);
        if !command_proofs.is_empty() {
            for spec in command_proofs {
                repository_command_proofs.insert(spec.id);
                changed_packages.extend(
                    spec.workspace_packages
                        .iter()
                        .map(|name| (*name).to_owned()),
                );
                for suite in spec.heavy_suites {
                    let suite = match suite {
                        HeavySuite::Browser => "browser",
                        HeavySuite::Conduitos => "conduitos",
                        HeavySuite::Esp32 => "esp32",
                    };
                    selected.insert(suite.to_owned(), true);
                    reasons
                        .get_mut(suite)
                        .expect("known suite")
                        .push(format!("repository-command:{}:{path}", spec.id));
                }
            }
            continue;
        }
        let presentation_proofs = browser_presentation_proofs_for_path(path);
        if !presentation_proofs.is_empty() {
            selected.insert("browser".to_owned(), true);
            for spec in presentation_proofs {
                reasons
                    .get_mut("browser")
                    .expect("known suite")
                    .push(format!("product-proof:{}:{path}", spec.id));
            }
            if let Some(package) = package_for_path(root, path, packages) {
                changed_packages.insert(package.to_owned());
            }
            continue;
        }
        if tongues_analysis_slice {
            selected.insert("browser".to_owned(), true);
            reasons
                .get_mut("browser")
                .expect("known suite")
                .push(format!("focused-tongues-analysis:{path}"));
            if let Some(package) = package_for_path(root, path, packages) {
                changed_packages.insert(package.to_owned());
            }
            continue;
        }
        if creche_presentation_slice {
            selected.insert("browser".to_owned(), true);
            reasons
                .get_mut("browser")
                .expect("known suite")
                .push(format!("focused-creche-presentation:{path}"));
            if let Some(package) = package_for_path(root, path, packages) {
                changed_packages.insert(package.to_owned());
            }
            continue;
        }
        if pi_zero_creche_slice {
            selected.insert("browser".to_owned(), true);
            reasons
                .get_mut("browser")
                .expect("known suite")
                .push(format!("focused-pi-zero-creche:{path}"));
            if let Some(package) = package_for_path(root, path, packages) {
                changed_packages.insert(package.to_owned());
            }
            continue;
        }
        if FOCUSED_WORKFLOW_FILES.contains(&path.as_str()) {
            selected.insert("browser".to_owned(), true);
            reasons
                .get_mut("browser")
                .expect("known suite")
                .push(format!("focused-workflow:{path}"));
            continue;
        }
        if debugger_kernel_slice && DEBUGGER_KERNEL_SLICE.contains(&path.as_str()) {
            selected.insert("browser".to_owned(), true);
            reasons
                .get_mut("browser")
                .expect("known suite")
                .push(format!("focused-debugger-kernel:{path}"));
            continue;
        }
        if patchbay_package_slice && path.as_str() == "Cargo.lock" {
            selected.insert("browser".to_owned(), true);
            reasons
                .get_mut("browser")
                .expect("known suite")
                .push("focused-patchbay-package:Cargo.lock".to_owned());
            continue;
        }
        if path == "Cargo.lock" && manifest_backed_lock {
            for suite in SUITES {
                reasons
                    .get_mut(suite)
                    .expect("known suite")
                    .push("manifest-backed-lock:Cargo.lock".to_owned());
            }
            continue;
        }
        if path == ".github/workflows/check.yml" {
            check_workflow_changed = true;
            continue;
        }
        if is_repository_tool_test(path) {
            changed_packages.insert("xtask".to_owned());
            continue;
        }
        if GLOBAL_FILES.contains(&path.as_str()) || starts_with_any(path, &GLOBAL_PREFIXES) {
            return Ok(full_plan(format!("global-change:{path}"), paths));
        }
        let mut direct = false;
        for suite in SUITES {
            if starts_with_any(path, direct_prefixes(suite)) {
                selected.insert(suite.to_owned(), true);
                reasons
                    .get_mut(suite)
                    .expect("known suite")
                    .push(format!("owned-path:{path}"));
                direct = true;
                if suite == "conduitos" {
                    select_conduitos_path(path, &mut conduitos);
                } else if suite == "esp32" {
                    select_esp32_path(path, &mut esp32);
                }
            }
        }
        if let Some(package) = package_for_path(root, path, packages) {
            changed_packages.insert(package.to_owned());
            for suite in SUITES {
                if closures[suite].contains(package)
                    && machine_proof_is_required_for_dependency(path, suite)
                {
                    selected.insert(suite.to_owned(), true);
                    reasons
                        .get_mut(suite)
                        .expect("known suite")
                        .push(format!("package-dependency:{package}"));
                    if suite == "conduitos" && !starts_with_any(path, direct_prefixes("conduitos"))
                    {
                        conduitos = ConduitosImpact::all();
                    } else if suite == "esp32" && !starts_with_any(path, direct_prefixes("esp32")) {
                        esp32 = Esp32Impact::all();
                    }
                }
            }
        } else if !direct
            && !HARMLESS_FILES.contains(&path.as_str())
            && !starts_with_any(path, &HARMLESS_PREFIXES)
        {
            return Ok(full_plan(format!("unclassified-path:{path}"), paths));
        }
    }
    if check_workflow_changed {
        for suite in SUITES {
            selected.insert(suite.to_owned(), true);
            reasons
                .get_mut(suite)
                .expect("known suite")
                .push("check-workflow-implementation".to_owned());
        }
        esp32 = Esp32Impact::all();
        conduitos = ConduitosImpact::all();
    }
    let names: Vec<_> = SUITES
        .into_iter()
        .filter(|suite| selected[*suite])
        .collect();
    let reason = if names.is_empty() {
        "no-heavyweight-obligations".to_owned()
    } else {
        format!("selected:{}", names.join(","))
    };
    let affected = affected_tests(packages, &changed_packages);
    let lint_packages = changed_packages
        .iter()
        .filter(|name| packages[*name].workspace_member)
        .cloned()
        .collect();
    let workspace_shards = if check_workflow_changed {
        WorkspaceShard::ALL
            .into_iter()
            .map(|shard| (shard.name().to_owned(), true))
            .collect()
    } else if changed_packages.is_empty() && controller_only {
        WorkspaceShard::ALL
            .into_iter()
            .map(|shard| (shard.name().to_owned(), false))
            .collect()
    } else {
        workspace_shards_for(root, packages, &affected)?
    };
    selected.insert("esp32".to_owned(), esp32.required());
    selected.insert("conduitos".to_owned(), conduitos.required());
    let mut result = plan(
        selected,
        MachineImpact { esp32, conduitos },
        false,
        reason,
        paths,
        WorkspaceImpact {
            changed_packages,
            affected_test_packages: affected,
            lint_packages,
            shards: workspace_shards,
        },
        reasons,
    );
    result.repository_command_proofs = repository_command_proofs.into_iter().collect();
    if check_workflow_changed {
        result.reason = "check-workflow-implementation".to_owned();
        result.workspace_lint_full = true;
        result.workspace_lint_packages.clear();
    }
    Ok(result)
}

fn select_esp32_path(path: &str, impact: &mut Esp32Impact) {
    if path.starts_with("targets/esp32/fabrication/")
        || path == "targets/esp32/README.md"
        || path.starts_with("targets/esp32/firmware/wroom-signal/src/")
    {
        *impact = Esp32Impact::all();
    } else if path.starts_with("targets/esp32/firmware/c3-signal/") {
        impact.targets.insert("c3".to_owned());
    } else if path.starts_with("targets/esp32/firmware/s3-signal/") {
        impact.targets.insert("s3".to_owned());
    } else if path.starts_with("targets/esp32/firmware/wroom-signal/") {
        impact.targets.insert("wroom".to_owned());
    } else {
        *impact = Esp32Impact::all();
    }
}

fn select_conduitos_path(path: &str, impact: &mut ConduitosImpact) {
    if path == "targets/conduitos/src/bin/aarch64_product.rs"
        || path == "profiles/hosts/conduitos-aarch64-headless.profile.json"
    {
        impact.aarch64_product = true;
        return;
    }
    if path.starts_with("targets/conduitos/src/arch/x86_64/xhci") {
        // Every retained x86 appliance boots through the shared xHCI-enabled
        // image, including the front-door and product-journey keyboard paths.
        impact
            .x86_proofs
            .extend(CONDUITOS_X86_PROOFS.map(str::to_owned));
        return;
    }
    if path.starts_with("targets/conduitos/src/arch/x86_64/") {
        impact
            .x86_proofs
            .extend(CONDUITOS_X86_PROOFS.map(str::to_owned));
        return;
    }
    for architecture in CONDUITOS_ARCHITECTURES {
        if path.starts_with(&format!(
            "targets/conduitos/proof-appliances/{architecture}/"
        )) {
            impact.architectures.insert(architecture.to_owned());
            return;
        }
        if path.starts_with(&format!("targets/conduitos/src/arch/{architecture}/")) {
            impact.architectures.insert(architecture.to_owned());
            if architecture == "aarch64" {
                impact.aarch64_product = true;
            }
            return;
        }
        if path.starts_with(&format!(
            "targets/conduitos/fabrication/xtask/{architecture}_"
        )) {
            impact.architectures.insert(architecture.to_owned());
            return;
        }
    }
    *impact = ConduitosImpact::all();
}

fn workspace_shards_for(
    root: &Path,
    packages: &BTreeMap<String, Package>,
    affected: &BTreeSet<String>,
) -> Result<BTreeMap<String, bool>, String> {
    WorkspaceShard::ALL
        .into_iter()
        .map(|shard| {
            let roots = workspace_obligation_roots(root, packages, shard)?;
            let required = shard == WorkspaceShard::Lint
                || roots.iter().any(|package| affected.contains(package));
            Ok((shard.name().to_owned(), required))
        })
        .collect()
}

fn workspace_obligation_roots(
    root: &Path,
    packages: &BTreeMap<String, Package>,
    shard: WorkspaceShard,
) -> Result<BTreeSet<String>, String> {
    let mut roots: BTreeSet<String> = shard
        .test_packages()
        .iter()
        .map(|package| (*package).to_owned())
        .collect();
    for step in WORKSPACE_STEPS
        .iter()
        .chain(NETWORK_CAPABILITY_STEPS)
        .chain(PICO_COMPOSITION_STEPS)
        .filter(|step| shard.owns(step))
    {
        let mut arguments = step.args.iter();
        while let Some(argument) = arguments.next() {
            if *argument == "-p" {
                roots.insert(
                    arguments
                        .next()
                        .ok_or_else(|| format!("{} omits package after -p", step.id))?
                        .to_string(),
                );
            } else if *argument == "--manifest-path" {
                let manifest = arguments
                    .next()
                    .ok_or_else(|| format!("{} omits path after --manifest-path", step.id))?;
                let package = package_for_path(root, manifest, packages)
                    .ok_or_else(|| format!("{} manifest has no discovered package", step.id))?;
                roots.insert(package.to_owned());
            }
        }
    }
    Ok(roots)
}

fn package_for_path<'a>(
    root: &Path,
    path: &str,
    packages: &'a BTreeMap<String, Package>,
) -> Option<&'a str> {
    let absolute = normalize(&root.join(path));
    packages
        .values()
        .filter(|package| absolute.starts_with(&package.directory))
        .max_by_key(|package| package.directory.components().count())
        .map(|package| package.name.as_str())
}

fn starts_with_any(path: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| path.starts_with(prefix))
}

fn plan(
    selected: BTreeMap<String, bool>,
    machine: MachineImpact,
    full_fallback: bool,
    reason: String,
    changed_paths: Vec<String>,
    workspace: WorkspaceImpact,
    suite_reasons: BTreeMap<String, Vec<String>>,
) -> ImpactPlan {
    let shared_eligible_packages = workspace.lint_packages.clone();
    let workspace_lint_packages = if full_fallback {
        Vec::new()
    } else {
        workspace.lint_packages.into_iter().collect()
    };
    let pages_product_proofs = product_proofs_for_paths(&changed_paths);
    let pages_products_required = full_fallback || !pages_product_proofs.is_empty();
    let selected_workspace_shards = workspace
        .shards
        .values()
        .filter(|required| **required)
        .count();
    let selected_heavy_worlds = [
        pages_products_required,
        selected["browser"],
        selected["esp32"],
        selected["conduitos"],
    ]
    .into_iter()
    .filter(|required| *required)
    .count();
    // A changed workspace package consumed by more than one selected proof
    // world is compiled once before those worlds fan out. Unknown/global
    // changes retain the existing conservative proof graph rather than
    // pretending that one package compile covers them.
    let shared_compile_packages =
        if !full_fallback && selected_workspace_shards + selected_heavy_worlds > 1 {
            shared_eligible_packages.into_iter().collect()
        } else {
            Vec::new()
        };
    ImpactPlan {
        requested_base_sha: None,
        candidate_sha: None,
        candidate_comparison_base_sha: None,
        ci_controller_proofs: controller_proofs(&changed_paths),
        repository_command_proofs: Vec::new(),
        pages_product_proofs,
        pages_products_required,
        esp32_required: selected["esp32"],
        esp32_targets: machine.esp32.targets.into_iter().collect(),
        browser_required: selected["browser"],
        conduitos_required: selected["conduitos"],
        conduitos_x86_proofs: machine.conduitos.x86_proofs.into_iter().collect(),
        conduitos_architectures: machine.conduitos.architectures.into_iter().collect(),
        conduitos_aarch64_product_required: machine.conduitos.aarch64_product,
        full_fallback,
        reason,
        changed_paths,
        changed_packages: workspace.changed_packages.into_iter().collect(),
        affected_test_packages: workspace.affected_test_packages.into_iter().collect(),
        shared_compile_packages,
        workspace_lint_full: full_fallback,
        workspace_lint_packages,
        workspace_shards: workspace.shards,
        suite_reasons,
    }
}

fn empty_reasons() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), Vec::new())))
}

fn full_plan(reason: String, paths: Vec<String>) -> ImpactPlan {
    let selected = BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), true)));
    let suite_reasons =
        BTreeMap::from(SUITES.map(|suite| (suite.to_owned(), vec![reason.clone()])));
    plan(
        selected,
        MachineImpact {
            esp32: Esp32Impact::all(),
            conduitos: ConduitosImpact::all(),
        },
        true,
        reason,
        paths,
        WorkspaceImpact {
            changed_packages: BTreeSet::new(),
            affected_test_packages: BTreeSet::new(),
            lint_packages: BTreeSet::new(),
            shards: WorkspaceShard::ALL
                .into_iter()
                .map(|shard| (shard.name().to_owned(), true))
                .collect(),
        },
        suite_reasons,
    )
}

fn write_github_outputs(plan: &ImpactPlan) {
    println!(
        "ci_controller_required={}",
        !plan.ci_controller_proofs.is_empty()
    );
    println!(
        "ci_controller_proofs={}",
        serde_json::to_string(&plan.ci_controller_proofs)
            .expect("CI controller proof list serializes")
    );
    println!("pages_products_required={}", plan.pages_products_required);
    println!(
        "pages_product_proofs={}",
        serde_json::to_string(&plan.pages_product_proofs)
            .expect("Pages product proof list serializes")
    );
    println!("esp32_required={}", plan.esp32_required);
    println!(
        "esp32_matrix={}",
        serde_json::to_string(&plan.esp32_targets).expect("ESP32 target matrix serializes")
    );
    println!("browser_required={}", plan.browser_required);
    println!("conduitos_required={}", plan.conduitos_required);
    println!(
        "conduitos_x86_matrix={}",
        serde_json::to_string(&plan.conduitos_x86_proofs).expect("ConduitOS x86 matrix serializes")
    );
    println!(
        "conduitos_architecture_matrix={}",
        serde_json::to_string(&plan.conduitos_architectures)
            .expect("ConduitOS architecture matrix serializes")
    );
    println!(
        "conduitos_aarch64_product_required={}",
        plan.conduitos_aarch64_product_required
    );
    println!("full_fallback={}", plan.full_fallback);
    println!("impact_reason={}", plan.reason);
    println!("workspace_lint_full={}", plan.workspace_lint_full);
    println!(
        "workspace_lint_packages={}",
        serde_json::to_string(&plan.workspace_lint_packages)
            .expect("workspace lint package list serializes")
    );
    println!(
        "workspace_test_packages={}",
        serde_json::to_string(&plan.affected_test_packages)
            .expect("workspace test package list serializes")
    );
    println!(
        "shared_compile_packages={}",
        serde_json::to_string(&plan.shared_compile_packages)
            .expect("shared compile package list serializes")
    );
    let matrix: Vec<_> = WorkspaceShard::ALL
        .into_iter()
        .filter(|shard| plan.workspace_shards[shard.name()])
        .map(WorkspaceShard::name)
        .collect();
    println!(
        "workspace_matrix={}",
        serde_json::to_string(&matrix).expect("workspace matrix serializes")
    );
}

fn markdown_summary(plan: &ImpactPlan) -> String {
    let mut rows = String::new();
    let controller_reason = if plan.ci_controller_proofs.is_empty() {
        "no controller implementation input".to_owned()
    } else {
        plan.ci_controller_proofs.join(", ")
    };
    rows.push_str(&format!(
        "| ci/controller | {} | {} |\n",
        if plan.ci_controller_proofs.is_empty() {
            "skip"
        } else {
            "run"
        },
        controller_reason
    ));
    rows.push_str(&format!(
        "| products/pages | {} | {} |\n",
        if plan.pages_products_required {
            "run"
        } else {
            "skip on PR"
        },
        if plan.pages_product_proofs.is_empty() {
            if plan.full_fallback {
                "conservative global fallback".to_owned()
            } else {
                "no product input".to_owned()
            }
        } else {
            plan.pages_product_proofs.join(", ")
        }
    ));
    for suite in SUITES {
        let reasons = &plan.suite_reasons[suite];
        let why = if reasons.is_empty() {
            "no dependency/ownership path".to_owned()
        } else {
            reasons.join(", ")
        };
        rows.push_str(&format!(
            "| {suite} | {} | {why} |\n",
            if required(plan, suite) {
                "run"
            } else {
                "skip on PR"
            }
        ));
    }
    for target in ESP32_TARGETS {
        let selected = plan
            .esp32_targets
            .iter()
            .any(|candidate| candidate == target);
        rows.push_str(&format!(
            "| esp32/{target} | {} | {} |\n",
            if selected { "run" } else { "skip on PR" },
            if selected {
                "affected target or shared family dependency"
            } else {
                "no dependency/ownership path to target"
            }
        ));
    }
    for shard in WorkspaceShard::ALL {
        rows.push_str(&format!(
            "| workspace/{} | {} | {} |\n",
            shard.name(),
            if plan.workspace_shards[shard.name()] {
                "run"
            } else {
                "skip on PR"
            },
            if shard == WorkspaceShard::Lint {
                "permanent product spine"
            } else if plan.workspace_shards[shard.name()] {
                "affected reverse-dependent package"
            } else {
                "no affected package assigned to shard"
            }
        ));
    }
    for proof in CONDUITOS_X86_PROOFS {
        let selected = plan
            .conduitos_x86_proofs
            .iter()
            .any(|candidate| candidate == proof);
        rows.push_str(&format!(
            "| conduitos/x86/{proof} | {} | {} |\n",
            if selected { "run" } else { "skip on PR" },
            if selected {
                "affected ConduitOS x86 claim"
            } else {
                "no dependency/ownership path to proof"
            }
        ));
    }
    for architecture in CONDUITOS_ARCHITECTURES {
        let selected = plan
            .conduitos_architectures
            .iter()
            .any(|candidate| candidate == architecture);
        rows.push_str(&format!(
            "| conduitos/architecture/{architecture} | {} | {} |\n",
            if selected { "run" } else { "skip on PR" },
            if selected {
                "affected architecture claim"
            } else {
                "no dependency/ownership path to architecture"
            }
        ));
    }
    rows.push_str(&format!(
        "| conduitos/aarch64-product | {} | {} |\n",
        if plan.conduitos_aarch64_product_required {
            "run"
        } else {
            "skip on PR"
        },
        if plan.conduitos_aarch64_product_required {
            "affected product claim"
        } else {
            "no dependency/ownership path to product"
        }
    ));
    format!("## CI impact plan\n\nRequested base: `{}`  \nCandidate: `{}`  \nCandidate comparison base: `{}`\n\nReason: `{}`\n\nChanged packages: {}\n\nAffected test packages: {}\n\nShared compile prerequisite: {}\n\n| obligation | decision | reason |\n| --- | --- | --- |\n{}\n> Pull requests may use this plan selectively. Main and merge-queue runs remain exhaustive.\n", plan.requested_base_sha.as_deref().unwrap_or("unavailable"), plan.candidate_sha.as_deref().unwrap_or("unavailable"), plan.candidate_comparison_base_sha.as_deref().unwrap_or("unavailable; conservative fallback"), plan.reason, if plan.changed_packages.is_empty() { "(none)".to_owned() } else { plan.changed_packages.join(", ") }, if plan.affected_test_packages.is_empty() { "(none)".to_owned() } else { plan.affected_test_packages.join(", ") }, if plan.shared_compile_packages.is_empty() { "not required".to_owned() } else { plan.shared_compile_packages.join(", ") }, rows)
}

fn required(plan: &ImpactPlan, suite: &str) -> bool {
    match suite {
        "esp32" => plan.esp32_required,
        "browser" => plan.browser_required,
        "conduitos" => plan.conduitos_required,
        _ => false,
    }
}

fn write_parent(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "impact/tests.rs"]
mod tests;

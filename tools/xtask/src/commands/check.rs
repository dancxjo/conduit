use clap::{Args, ValueEnum};
use std::collections::BTreeSet;

#[path = "../suites/todo.rs"]
mod todo;

use crate::{
    cli::GlobalOpts,
    process::{run_step, run_step_with_arguments, run_suite, Step, StepError},
    suites::check::{
        BROWSER_CHECK_STEPS, FORM_S3_STEPS, INPUT_SEMANTICS_STEPS, KERNEL_TAKEOVER_STEPS,
        OBSERVATORY_READINESS_STEPS, PLANNING_S2_STEPS, SEMANTIC_CATALOG_READINESS_STEPS,
        SIM_READINESS_STEPS, WORKSPACE_STEPS,
    },
    suites::network_capability::NETWORK_CAPABILITY_STEPS,
    suites::pico_compositions::PICO_COMPOSITION_STEPS,
    suites::workspace_shards::WorkspaceShard,
    workspace::workspace_root,
};

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Which check suite to execute (default: workspace).
    #[arg(default_value = "workspace")]
    pub suite: CheckSuite,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSuite {
    Workspace,
    WorkspaceLint,
    WorkspaceTestFoundation,
    WorkspaceTestHosts,
    WorkspaceTestProducts,
    WorkspacePortable,
    WorkspacePico,
    Browser,
    BrowserHost,
    Sim,
    KernelTakeover,
    PlanningS2,
    FormS3,
    Observatory,
    SemanticCatalog,
    /// Execute authored quantity mappings through the std production kernel and presentation.
    QuantityMapping,
    /// Prove bounded Todo state transitions and recursive Form execution.
    TodoState,
    InputSemantics,
    All,
}

pub fn run(args: CheckArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;

    match args.suite {
        CheckSuite::Workspace => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(NETWORK_CAPABILITY_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)
        }
        CheckSuite::WorkspaceLint => run_workspace_shard(WorkspaceShard::Lint, &root, opts),
        CheckSuite::WorkspaceTestFoundation => {
            run_workspace_shard(WorkspaceShard::TestFoundation, &root, opts)
        }
        CheckSuite::WorkspaceTestHosts => {
            run_workspace_shard(WorkspaceShard::TestHosts, &root, opts)
        }
        CheckSuite::WorkspaceTestProducts => {
            run_workspace_shard(WorkspaceShard::TestProducts, &root, opts)
        }
        CheckSuite::WorkspacePortable => run_workspace_shard(WorkspaceShard::Portable, &root, opts),
        CheckSuite::WorkspacePico => run_workspace_shard(WorkspaceShard::Pico, &root, opts),
        CheckSuite::Browser | CheckSuite::BrowserHost => {
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
        CheckSuite::Sim => run_suite(SIM_READINESS_STEPS, &root, opts),
        CheckSuite::KernelTakeover => run_suite(KERNEL_TAKEOVER_STEPS, &root, opts),
        CheckSuite::PlanningS2 => run_suite(PLANNING_S2_STEPS, &root, opts),
        CheckSuite::FormS3 => run_suite(FORM_S3_STEPS, &root, opts),
        CheckSuite::Observatory => run_suite(OBSERVATORY_READINESS_STEPS, &root, opts),
        CheckSuite::SemanticCatalog => run_suite(SEMANTIC_CATALOG_READINESS_STEPS, &root, opts),
        CheckSuite::QuantityMapping => run_suite(QUANTITY_MAPPING_STEPS, &root, opts),
        CheckSuite::TodoState => run_suite(todo::TODO_STATE_STEPS, &root, opts),
        CheckSuite::InputSemantics => run_suite(INPUT_SEMANTICS_STEPS, &root, opts),
        CheckSuite::All => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(NETWORK_CAPABILITY_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)?;
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
    }
}

const QUANTITY_MAPPING_STEPS: &[Step] = &[
    Step::new(
        "quantity-mapping.browser-build",
        "Build the actual browser quantity runtime",
        "cargo",
        &[
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--features",
            "tour-surface",
            "--locked",
        ],
    ),
    Step::new(
        "quantity-mapping.contract",
        "Check exact mapping refusals and bounded structured Quantity encoding",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-semantic-catalog",
            "--all-features",
            "--locked",
            "quantity_",
        ],
    ),
    Step::new(
        "quantity-mapping.kernel",
        "Execute authored quantity Forms with output, correlated Signs and connected refusals",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--lib",
            "--locked",
            "quantity_",
        ],
    ),
    Step::new(
        "quantity-mapping.browser-runtime",
        "Execute authored quantity Forms through browser kernel and typed output effects",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-browser-runtime",
            "--lib",
            "--locked",
            "quantity",
        ],
    ),
    Step::new(
        "quantity-mapping.inventory-bound",
        "Preserve bounded browser inventory navigation as installed offers grow",
        "node",
        &["--test", "proof/browser/tour-inventory-pagination.test.mjs"],
    ),
    Step::new(
        "quantity-mapping.chromium",
        "Prove real pointer causality and deterministic alternate input in pinned Chromium",
        "node",
        &[
            "proof/browser/node_modules/@playwright/test/cli.js",
            "test",
            "--config",
            "proof/browser/playwright.config.mjs",
            "--project=chromium",
            "quantity-controller.spec.mjs",
        ],
    ),
];

fn run_workspace_shard(
    shard: WorkspaceShard,
    root: &std::path::Path,
    opts: &GlobalOpts,
) -> Result<(), StepError> {
    let planned_tests = planned_packages("CONDUIT_CI_TEST_PACKAGES")?;
    if let Some(step) = shard.package_test_step() {
        if let Some(packages) = planned_tests.as_ref() {
            if let Some(arguments) = selective_package_test_arguments(step, packages)? {
                run_step_with_arguments(step, &arguments, root, opts)?;
            }
        } else {
            run_step(step, root, opts)?;
        }
    }
    for step in WORKSPACE_STEPS
        .iter()
        .chain(NETWORK_CAPABILITY_STEPS)
        .chain(PICO_COMPOSITION_STEPS)
        .filter(|step| shard.owns(step))
    {
        if shard == WorkspaceShard::Lint && step.id == "check.clippy" {
            if let Some(packages) = planned_packages("CONDUIT_CI_LINT_PACKAGES")? {
                if packages.is_empty() {
                    continue;
                }
                run_step_with_arguments(step, &selective_clippy_arguments(&packages), root, opts)?;
                continue;
            }
        }
        run_step(step, root, opts)?;
    }
    Ok(())
}

fn planned_packages(variable: &str) -> Result<Option<BTreeSet<String>>, StepError> {
    let Some(full) = std::env::var_os("CONDUIT_CI_LINT_FULL") else {
        return Ok(None);
    };
    match full.to_str() {
        Some("true") => return Ok(None),
        Some("false") => {}
        _ => {
            return Err(StepError::prereq(
                "check.clippy.plan",
                "CONDUIT_CI_LINT_FULL must be true or false",
            ));
        }
    }
    let encoded = std::env::var(variable).map_err(|_| {
        StepError::prereq(
            "check.workspace-plan",
            format!("selective workspace checks require {variable}"),
        )
    })?;
    let packages: BTreeSet<String> = serde_json::from_str(&encoded).map_err(|error| {
        StepError::prereq(
            "check.workspace-plan",
            format!("invalid package JSON in {variable}: {error}"),
        )
    })?;
    if packages.iter().any(String::is_empty) {
        return Err(StepError::prereq(
            "check.clippy.plan",
            "lint package identities must not be empty",
        ));
    }
    Ok(Some(packages))
}

fn selective_clippy_arguments(packages: &BTreeSet<String>) -> Vec<String> {
    let mut arguments = vec!["clippy".to_owned()];
    for package in packages {
        arguments.extend(["-p".to_owned(), package.clone()]);
    }
    arguments.extend(
        ["--all-targets", "--", "-D", "warnings"]
            .into_iter()
            .map(str::to_owned),
    );
    arguments
}

fn selective_package_test_arguments(
    step: &Step,
    selected: &BTreeSet<String>,
) -> Result<Option<Vec<String>>, StepError> {
    let mut arguments = Vec::new();
    let mut selected_count = 0;
    let mut index = 0;
    while index < step.args.len() {
        match step.args[index] {
            "-p" => {
                let package = step.args.get(index + 1).ok_or_else(|| {
                    StepError::prereq(step.id, "package test step omits package after -p")
                })?;
                if selected.contains(*package) {
                    arguments.extend(["-p".to_owned(), (*package).to_owned()]);
                    selected_count += 1;
                }
                index += 2;
            }
            "--features" => {
                let features = step.args.get(index + 1).ok_or_else(|| {
                    StepError::prereq(step.id, "package test step omits --features value")
                })?;
                let retained = features
                    .split(',')
                    .filter(|feature| {
                        feature
                            .split_once('/')
                            .is_none_or(|(package, _)| selected.contains(package))
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                if !retained.is_empty() {
                    arguments.extend(["--features".to_owned(), retained]);
                }
                index += 2;
            }
            argument => {
                arguments.push(argument.to_owned());
                index += 1;
            }
        }
    }
    Ok((selected_count != 0).then_some(arguments))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{selective_clippy_arguments, selective_package_test_arguments};
    use crate::suites::workspace_shards::WorkspaceShard;

    #[test]
    fn selective_clippy_keeps_exact_packages_and_warning_gate() {
        assert_eq!(
            selective_clippy_arguments(&BTreeSet::from([
                "conduit-pete".to_owned(),
                "xtask".to_owned(),
            ])),
            [
                "clippy",
                "-p",
                "conduit-pete",
                "-p",
                "xtask",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ]
        );
    }

    #[test]
    fn selective_package_tests_keep_only_planned_packages_and_owned_features() {
        let step = WorkspaceShard::TestProducts.package_test_step().unwrap();
        let arguments = selective_package_test_arguments(
            step,
            &BTreeSet::from(["conduit-pete".to_owned(), "xtask".to_owned()]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(arguments, ["test", "-p", "conduit-pete", "-p", "xtask"]);

        let tongues =
            selective_package_test_arguments(step, &BTreeSet::from(["conduit-tongues".to_owned()]))
                .unwrap()
                .unwrap();
        assert_eq!(
            tongues,
            [
                "test",
                "-p",
                "conduit-tongues",
                "--features",
                "conduit-tongues/speech",
            ]
        );
    }
}

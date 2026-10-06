//! Workspace shard execution and selective package argument construction.
use crate::suites::host_protocol_shards;
use std::collections::BTreeSet;

use crate::{
    cli::GlobalOpts,
    process::{run_step, run_step_with_arguments, Step, StepError},
    suites::{
        check::WORKSPACE_STEPS, network_capability::NETWORK_CAPABILITY_STEPS,
        pico_compositions::PICO_COMPOSITION_STEPS, workspace_shards::WorkspaceShard,
    },
};

pub(super) fn run_workspace_shard(
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
    if host_protocol_shards::is_host_group(shard)
        && planned_tests
            .as_ref()
            .is_none_or(|packages| packages.contains(host_protocol_shards::PACKAGE))
        && host_protocol_shards::selected()
            .map_err(|error| StepError::prereq("check.test.conduitos.plan", error))?
    {
        if shard == WorkspaceShard::TestHostsConduitos && !opts.dry_run {
            host_protocol_shards::validate_library_partition()
                .map_err(|error| StepError::prereq("check.test.conduitos.inventory", error))?;
        }
        for proof in host_protocol_shards::proofs(shard)
            .map_err(|error| StepError::prereq("check.test.conduitos.plan", error))?
        {
            run_step_with_arguments(proof.step, &proof.arguments, root, opts)?;
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
        assert_eq!(
            arguments,
            [
                "test",
                "--no-fail-fast",
                "-p",
                "conduit-pete",
                "-p",
                "xtask"
            ]
        );

        let tongues =
            selective_package_test_arguments(step, &BTreeSet::from(["conduit-tongues".to_owned()]))
                .unwrap()
                .unwrap();
        assert_eq!(
            tongues,
            [
                "test",
                "--no-fail-fast",
                "-p",
                "conduit-tongues",
                "--features",
                "conduit-tongues/speech",
            ]
        );
    }
}

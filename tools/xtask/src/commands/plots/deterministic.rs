//! Isolated execution of inventory-declared deterministic proof oracles.

use super::{result, DeterministicOracle, InventoryPlot, PlotProofResult, ReusablePlot};
use crate::cli::GlobalOpts;
use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Instant;

const MAXIMUM_FAILURE_REASON_BYTES: usize = 512;
const EVIDENCE_MARKER: &str = "CONDUIT_PLOT_EVIDENCE=";

#[derive(Deserialize)]
struct ExecutionEvidence {
    plan_id: String,
    play_id: String,
    workload_revision: Option<u64>,
}

pub(super) fn availability(
    plot: &InventoryPlot,
    path: &str,
    identities: Option<(String, String)>,
) -> PlotProofResult {
    match (&plot.deterministic, &plot.deterministic_not_applicable) {
        (Some(oracle), None) => result(
            plot,
            path,
            0,
            "unavailable",
            &format!(
                "declared deterministic oracle is available through {}",
                command(oracle)
            ),
            identities,
            "deterministic",
        ),
        (None, Some(reason)) => result(
            plot,
            path,
            0,
            "not_applicable",
            reason,
            identities,
            "deterministic",
        ),
        (None, None) => result(
            plot,
            path,
            0,
            "unavailable",
            "no reviewed deterministic execution oracle is declared",
            identities,
            "deterministic",
        ),
        (Some(_), Some(_)) => result(
            plot,
            path,
            0,
            "refused",
            "inventory declares both a deterministic oracle and not-applicable reason",
            identities,
            "deterministic",
        ),
    }
}

pub(super) fn run(
    root: &Path,
    plot: &InventoryPlot,
    path: &str,
    identities: Option<(String, String)>,
    opts: &GlobalOpts,
) -> PlotProofResult {
    let Some(oracle) = &plot.deterministic else {
        return availability(plot, path, identities);
    };
    if plot.deterministic_not_applicable.is_some() {
        return availability(plot, path, identities);
    }
    if opts.dry_run {
        return result(
            plot,
            path,
            0,
            "unavailable",
            &format!("dry run planned: {}", command(oracle)),
            identities,
            "deterministic",
        );
    }
    execute(root, plot, path, identities, oracle, opts, "deterministic")
}

pub(super) fn reusable_availability(
    plot: &InventoryPlot,
    reusable: &ReusablePlot,
    path: &str,
    identities: Option<(String, String)>,
) -> PlotProofResult {
    let mut proof = match &reusable.deterministic {
        Some(oracle) => result(
            plot,
            path,
            0,
            "unavailable",
            &format!(
                "declared reusable deterministic oracle is available through {}",
                command(oracle)
            ),
            identities,
            "reusable-deterministic",
        ),
        None => result(
            plot,
            path,
            0,
            "unavailable",
            "no reviewed independent deterministic execution oracle is declared",
            identities,
            "reusable-deterministic",
        ),
    };
    proof.title = reusable.title.clone();
    proof.plot_entry = reusable.entry.clone();
    proof
}

pub(super) fn run_reusable(
    root: &Path,
    plot: &InventoryPlot,
    reusable: &ReusablePlot,
    path: &str,
    identities: Option<(String, String)>,
    opts: &GlobalOpts,
) -> PlotProofResult {
    let Some(oracle) = &reusable.deterministic else {
        return reusable_availability(plot, reusable, path, identities);
    };
    if opts.dry_run {
        return reusable_availability(plot, reusable, path, identities);
    }
    let mut proof = execute(
        root,
        plot,
        path,
        identities,
        oracle,
        opts,
        "reusable-deterministic",
    );
    proof.title = reusable.title.clone();
    proof.plot_entry = reusable.entry.clone();
    proof
}

pub(super) fn execute(
    root: &Path,
    plot: &InventoryPlot,
    path: &str,
    identities: Option<(String, String)>,
    oracle: &DeterministicOracle,
    opts: &GlobalOpts,
    mode: &'static str,
) -> PlotProofResult {
    let started = Instant::now();
    let mut process = Command::new("cargo");
    process.current_dir(root).arg("test");
    if opts.locked {
        process.arg("--locked");
    }
    if !oracle.features.is_empty() {
        process.args(["--features", &oracle.features.join(",")]);
    }
    process.args(oracle_arguments(oracle));
    if oracle.plan_play_evidence {
        process.arg("--nocapture");
    }
    let output = process.output();
    let duration = started.elapsed().as_millis();
    match output {
        Ok(output) if output.status.success() && !ran_one_test(&output.stdout) => result(
            plot,
            path,
            duration,
            "failed",
            &format!(
                "{} succeeded without exactly one passing test",
                command(oracle)
            ),
            identities,
            mode,
        ),
        Ok(output) if output.status.success() => {
            let mut proof = result(
                plot,
                path,
                duration,
                "passed",
                &format!("declared deterministic oracle passed: {}", command(oracle)),
                identities,
                mode,
            );
            if oracle.plan_play_evidence {
                match evidence(&output) {
                    Ok(evidence) => {
                        proof.plan_id = Some(evidence.plan_id);
                        proof.play_id = Some(evidence.play_id);
                        proof.workload_revision = evidence.workload_revision;
                        if oracle.workload_revision_evidence && proof.workload_revision.is_none() {
                            proof.status = "failed".into();
                            proof.reason =
                                "deterministic oracle passed without exact workload revision evidence"
                                    .into();
                        }
                    }
                    Err(reason) => {
                        proof.status = "failed".into();
                        proof.reason = reason;
                    }
                }
            }
            proof
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            result(
                plot,
                path,
                duration,
                "failed",
                &bounded_reason(&format!("{} failed: {stderr}", command(oracle))),
                identities,
                mode,
            )
        }
        Err(error) => result(
            plot,
            path,
            duration,
            "unavailable",
            &format!(
                "cannot start declared deterministic oracle {}: {error}",
                command(oracle)
            ),
            identities,
            mode,
        ),
    }
}

fn command(oracle: &DeterministicOracle) -> String {
    let features = if oracle.features.is_empty() {
        String::new()
    } else {
        format!(" --features {}", oracle.features.join(","))
    };
    let evidence = if oracle.plan_play_evidence {
        " --nocapture"
    } else {
        ""
    };
    let target = if oracle.test == "lib" {
        "--lib".to_string()
    } else {
        format!("--test {}", oracle.test)
    };
    format!(
        "cargo test -p {}{} {} {} -- --exact{}",
        oracle.package, features, target, oracle.case, evidence
    )
}

fn oracle_arguments(oracle: &DeterministicOracle) -> Vec<String> {
    let mut arguments = vec!["-p".into(), oracle.package.clone()];
    if oracle.test == "lib" {
        arguments.push("--lib".into());
    } else {
        arguments.extend(["--test".into(), oracle.test.clone()]);
    }
    arguments.extend([oracle.case.clone(), "--".into(), "--exact".into()]);
    arguments
}

// Cargo succeeds even when --exact selects no tests. Proof requires execution.
fn ran_one_test(stdout: &[u8]) -> bool {
    String::from_utf8_lossy(stdout)
        .lines()
        .any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
}

fn evidence(output: &Output) -> Result<ExecutionEvidence, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let encoded = stdout
        .lines()
        .find_map(|line| line.split_once(EVIDENCE_MARKER).map(|(_, value)| value))
        .ok_or_else(|| {
            "deterministic oracle passed without exact plan/Play evidence".to_string()
        })?;
    serde_json::from_str(encoded)
        .map_err(|error| format!("deterministic oracle emitted malformed evidence: {error}"))
}

pub(super) fn bounded_reason(reason: &str) -> String {
    if reason.len() <= MAXIMUM_FAILURE_REASON_BYTES {
        return reason.into();
    }
    let mut start = reason.len() - MAXIMUM_FAILURE_REASON_BYTES;
    while !reason.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &reason[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plot() -> InventoryPlot {
        InventoryPlot {
            slug: "fixture".into(),
            title: "Fixture".into(),
            entry: "fixture".into(),
            reusable_entries: Vec::new(),
            initial_body_order: None,
            initial_body_face_profile: 0,
            workspace_catalog: true,
            deterministic: None,
            deterministic_not_applicable: None,
            browser_safe: None,
            browser_safe_not_applicable: None,
            graceful_fallback: None,
        }
    }

    #[test]
    fn absence_inapplicability_and_conflicting_declarations_remain_distinct() {
        let mut item = plot();
        assert_eq!(
            availability(&item, "plots/fixture/main.conduit", None).status,
            "unavailable"
        );
        item.deterministic_not_applicable = Some("device proof only".into());
        assert_eq!(
            availability(&item, "plots/fixture/main.conduit", None).status,
            "not_applicable"
        );
        item.deterministic = Some(DeterministicOracle {
            package: "fixture".into(),
            features: vec![],
            test: "fixture".into(),
            case: "fixture".into(),
            plan_play_evidence: false,
            workload_revision_evidence: false,
        });
        assert_eq!(
            availability(&item, "plots/fixture/main.conduit", None).status,
            "refused"
        );
    }

    #[test]
    fn theremin_library_oracle_selects_one_fully_qualified_test() {
        let oracle = DeterministicOracle {
            package: "conduit-browser-runtime".into(),
            features: vec![],
            test: "lib".into(),
            case: "creche::typed_workset_tests::phone_theremin_checks_with_exact_two_axis_browser_realizations".into(),
            plan_play_evidence: false,
            workload_revision_evidence: false,
        };
        let arguments = oracle_arguments(&oracle);
        assert_eq!(
            arguments,
            [
                "-p",
                "conduit-browser-runtime",
                "--lib",
                &oracle.case,
                "--",
                "--exact"
            ]
        );
        assert!(command(&oracle).contains(" --lib creche::typed_workset_tests::"));
        assert!(!command(&oracle).contains("--test lib"));
        assert!(!ran_one_test(
            b"running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 1 filtered out\n"
        ));
        assert!(ran_one_test(b"running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out\n"));
        let mut integration = oracle;
        integration.test = "integration".into();
        assert_eq!(
            &oracle_arguments(&integration)[2..4],
            ["--test", "integration"]
        );
    }

    #[test]
    fn failure_reason_keeps_the_bounded_diagnostic_tail() {
        let reason = format!("{}terminal cause", "x".repeat(MAXIMUM_FAILURE_REASON_BYTES));
        let bounded = bounded_reason(&reason);
        assert!(bounded.len() <= MAXIMUM_FAILURE_REASON_BYTES + '…'.len_utf8());
        assert!(bounded.ends_with("terminal cause"));
    }

    #[test]
    fn exact_plan_and_play_evidence_is_required_when_declared() {
        let output = Output {
            status: success_status(),
            stdout: b"CONDUIT_PLOT_EVIDENCE={\"plan_id\":\"plan/1\",\"play_id\":\"play/1\"}\n"
                .to_vec(),
            stderr: Vec::new(),
        };
        let parsed = evidence(&output).unwrap();
        assert_eq!(parsed.plan_id, "plan/1");
        assert_eq!(parsed.play_id, "play/1");
        assert_eq!(parsed.workload_revision, None);

        let missing = Output {
            status: success_status(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        assert!(evidence(&missing).is_err());
    }

    #[cfg(unix)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn success_status() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }
}

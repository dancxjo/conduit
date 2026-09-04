//! Isolated execution of inventory-declared deterministic proof oracles.

use super::{result, DeterministicOracle, FormProofResult, InventoryForm};
use crate::cli::GlobalOpts;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

const MAXIMUM_FAILURE_REASON_BYTES: usize = 512;

pub(super) fn availability(
    form: &InventoryForm,
    path: &str,
    identities: Option<(String, String)>,
) -> FormProofResult {
    match (&form.deterministic, &form.deterministic_not_applicable) {
        (Some(oracle), None) => result(
            form,
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
            form,
            path,
            0,
            "not_applicable",
            reason,
            identities,
            "deterministic",
        ),
        (None, None) => result(
            form,
            path,
            0,
            "unavailable",
            "no reviewed deterministic execution oracle is declared",
            identities,
            "deterministic",
        ),
        (Some(_), Some(_)) => result(
            form,
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
    form: &InventoryForm,
    path: &str,
    identities: Option<(String, String)>,
    opts: &GlobalOpts,
) -> FormProofResult {
    let Some(oracle) = &form.deterministic else {
        return availability(form, path, identities);
    };
    if form.deterministic_not_applicable.is_some() {
        return availability(form, path, identities);
    }
    if opts.dry_run {
        return result(
            form,
            path,
            0,
            "unavailable",
            &format!("dry run planned: {}", command(oracle)),
            identities,
            "deterministic",
        );
    }
    let started = Instant::now();
    let mut process = Command::new("cargo");
    process.current_dir(root).arg("test");
    if opts.locked {
        process.arg("--locked");
    }
    if !oracle.features.is_empty() {
        process.args(["--features", &oracle.features.join(",")]);
    }
    let output = process
        .args([
            "-p",
            &oracle.package,
            "--test",
            &oracle.test,
            &oracle.case,
            "--",
            "--exact",
        ])
        .output();
    let duration = started.elapsed().as_millis();
    match output {
        Ok(output) if output.status.success() => result(
            form,
            path,
            duration,
            "passed",
            &format!("declared deterministic oracle passed: {}", command(oracle)),
            identities,
            "deterministic",
        ),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            result(
                form,
                path,
                duration,
                "failed",
                &bounded_reason(&format!("{} failed: {stderr}", command(oracle))),
                identities,
                "deterministic",
            )
        }
        Err(error) => result(
            form,
            path,
            duration,
            "unavailable",
            &format!(
                "cannot start declared deterministic oracle {}: {error}",
                command(oracle)
            ),
            identities,
            "deterministic",
        ),
    }
}

fn command(oracle: &DeterministicOracle) -> String {
    let features = if oracle.features.is_empty() {
        String::new()
    } else {
        format!(" --features {}", oracle.features.join(","))
    };
    format!(
        "cargo test -p {}{} --test {} {} -- --exact",
        oracle.package, features, oracle.test, oracle.case
    )
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

    fn form() -> InventoryForm {
        InventoryForm {
            slug: "fixture".into(),
            title: "Fixture".into(),
            entry: "fixture".into(),
            reusable_entries: Vec::new(),
            initial_body_order: None,
            deterministic: None,
            deterministic_not_applicable: None,
            browser_safe: None,
            browser_safe_not_applicable: None,
        }
    }

    #[test]
    fn absence_inapplicability_and_conflicting_declarations_remain_distinct() {
        let mut item = form();
        assert_eq!(
            availability(&item, "forms/fixture/main.conduit", None).status,
            "unavailable"
        );
        item.deterministic_not_applicable = Some("device proof only".into());
        assert_eq!(
            availability(&item, "forms/fixture/main.conduit", None).status,
            "not_applicable"
        );
        item.deterministic = Some(DeterministicOracle {
            package: "fixture".into(),
            features: vec![],
            test: "fixture".into(),
            case: "fixture".into(),
        });
        assert_eq!(
            availability(&item, "forms/fixture/main.conduit", None).status,
            "refused"
        );
    }

    #[test]
    fn failure_reason_keeps_the_bounded_diagnostic_tail() {
        let reason = format!("{}terminal cause", "x".repeat(MAXIMUM_FAILURE_REASON_BYTES));
        let bounded = bounded_reason(&reason);
        assert!(bounded.len() <= MAXIMUM_FAILURE_REASON_BYTES + '…'.len_utf8());
        assert!(bounded.ends_with("terminal cause"));
    }
}

//! Fresh-process execution of inventory-declared browser-safe Form proofs.

use super::{
    catalogs, check_one, deterministic::bounded_reason, load_inventory, result, FormProofResult,
    InventoryForm, Report, REPORT_SCHEMA,
};
use crate::cli::GlobalOpts;
use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Instant;

const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

pub(super) enum Preparation {
    Ready,
    Unavailable(String),
    Failed(String),
}

#[derive(Deserialize)]
struct BrowserEvidence {
    plan_id: String,
    play_id: String,
}

pub(super) fn build_report(root: &Path, opts: &GlobalOpts) -> Result<Report, String> {
    let inventory = load_inventory(root)?;
    let catalogs = catalogs()?;
    let preparation = prepare(root, &inventory.forms, opts);
    let mut results = Vec::with_capacity(inventory.forms.len() * 2);
    for form in inventory.forms {
        let source_path = format!("forms/{}/main.conduit", form.slug);
        let started = Instant::now();
        match check_one(root, &source_path, &form.entry, &catalogs) {
            Ok((source_id, checked_id)) => {
                let identities = Some((source_id.clone(), checked_id.clone()));
                results.push(result(
                    &form,
                    &source_path,
                    started.elapsed().as_millis(),
                    "passed",
                    "canonical source parsed and checked through the standard semantic catalog",
                    identities.clone(),
                    "check",
                ));
                results.push(run(
                    root,
                    &form,
                    &source_path,
                    identities,
                    &preparation,
                    opts,
                ));
            }
            Err(reason) => results.push(result(
                &form,
                &source_path,
                started.elapsed().as_millis(),
                "failed",
                &reason,
                None,
                "check",
            )),
        }
    }
    Ok(Report {
        schema: REPORT_SCHEMA,
        inventory_schema: inventory.schema,
        results,
    })
}

pub(super) fn prepare(root: &Path, forms: &[InventoryForm], opts: &GlobalOpts) -> Preparation {
    if !forms.iter().any(|form| form.browser_safe.is_some()) {
        return Preparation::Ready;
    }
    if opts.dry_run {
        return Preparation::Unavailable(
            "dry run planned: build the browser runtime before isolated Chromium proofs".into(),
        );
    }
    let mut command = Command::new("cargo");
    command.current_dir(root).arg("build");
    if opts.locked {
        command.arg("--locked");
    }
    match command
        .args([
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .output()
    {
        Ok(output) if output.status.success() => Preparation::Ready,
        Ok(output) => Preparation::Failed(bounded_reason(&format!(
            "browser runtime preparation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))),
        Err(error) => {
            Preparation::Unavailable(format!("cannot start browser runtime preparation: {error}"))
        }
    }
}

pub(super) fn run(
    root: &Path,
    form: &InventoryForm,
    path: &str,
    identities: Option<(String, String)>,
    preparation: &Preparation,
    opts: &GlobalOpts,
) -> FormProofResult {
    let mut proof = match (&form.browser_safe, &form.browser_safe_not_applicable) {
        (None, Some(reason)) => result(
            form,
            path,
            0,
            "not_applicable",
            reason,
            identities,
            "browser-safe",
        ),
        (None, None) => result(
            form,
            path,
            0,
            "unavailable",
            "no reviewed browser-safe execution oracle is declared",
            identities,
            "browser-safe",
        ),
        (Some(_), Some(_)) => result(
            form,
            path,
            0,
            "refused",
            "inventory declares both a browser-safe oracle and not-applicable reason",
            identities,
            "browser-safe",
        ),
        (Some(oracle), None) => match preparation {
            Preparation::Unavailable(reason) => result(
                form,
                path,
                0,
                "unavailable",
                reason,
                identities,
                "browser-safe",
            ),
            Preparation::Failed(reason) => {
                result(form, path, 0, "failed", reason, identities, "browser-safe")
            }
            Preparation::Ready if opts.dry_run => result(
                form,
                path,
                0,
                "unavailable",
                "dry run planned: isolated Playwright Chromium proof",
                identities,
                "browser-safe",
            ),
            Preparation::Ready => execute(root, form, path, identities, oracle),
        },
    };
    proof.environment_profile = "playwright/chromium-1.62.0-worker1-retry0";
    proof
}

fn execute(
    root: &Path,
    form: &InventoryForm,
    path: &str,
    identities: Option<(String, String)>,
    oracle: &super::BrowserOracle,
) -> FormProofResult {
    let started = Instant::now();
    let output = Command::new("npx")
        .current_dir(root)
        .args([
            "playwright",
            "test",
            "--config",
            "proof/browser/playwright.config.mjs",
            &oracle.spec,
            "--project",
            "chromium",
            "--workers",
            "1",
            "--retries",
            "0",
            "--grep",
            &oracle.case,
        ])
        .output();
    let duration = started.elapsed().as_millis();
    match output {
        Ok(output) if output.status.success() => match evidence(&output) {
            Ok(evidence) => {
                let mut proof = result(
                    form,
                    path,
                    duration,
                    "passed",
                    "declared browser-safe oracle passed in fresh Chromium state",
                    identities,
                    "browser-safe",
                );
                proof.plan_id = Some(evidence.plan_id);
                proof.play_id = Some(evidence.play_id);
                proof.evidence_artifacts.push(oracle.spec.clone());
                proof
            }
            Err(reason) => result(
                form,
                path,
                duration,
                "failed",
                &reason,
                identities,
                "browser-safe",
            ),
        },
        Ok(output) => {
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let unavailable = diagnostic.contains("Executable doesn't exist")
                || diagnostic.contains("browserType.launch: Executable")
                || diagnostic.contains("Cannot find package '@playwright/test'");
            let refused = diagnostic.contains("Form start refused:")
                || diagnostic.contains("source interaction refused:");
            result(
                form,
                path,
                duration,
                if unavailable {
                    "unavailable"
                } else if refused {
                    "refused"
                } else {
                    "failed"
                },
                &bounded_reason(&format!("browser-safe oracle did not pass: {diagnostic}")),
                identities,
                "browser-safe",
            )
        }
        Err(error) => result(
            form,
            path,
            duration,
            "unavailable",
            &format!("cannot start Playwright browser-safe oracle: {error}"),
            identities,
            "browser-safe",
        ),
    }
}

fn evidence(output: &Output) -> Result<BrowserEvidence, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let encoded = stdout
        .lines()
        .find_map(|line| line.split_once(EVIDENCE_MARKER).map(|(_, value)| value))
        .ok_or_else(|| "browser-safe oracle passed without exact Plan/Play evidence".to_string())?;
    serde_json::from_str(encoded).map_err(|error| {
        format!("browser-safe oracle emitted malformed Plan/Play evidence: {error}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> InventoryForm {
        InventoryForm {
            slug: "fixture".into(),
            title: "Fixture".into(),
            entry: "fixture".into(),
            initial_body_order: None,
            deterministic: None,
            deterministic_not_applicable: None,
            browser_safe: None,
            browser_safe_not_applicable: None,
        }
    }

    #[test]
    fn absence_inapplicability_and_conflicting_declarations_remain_distinct() {
        let root = Path::new(".");
        let opts = GlobalOpts::default();
        let mut item = form();
        assert_eq!(
            run(
                root,
                &item,
                "forms/fixture/main.conduit",
                None,
                &Preparation::Ready,
                &opts,
            )
            .status,
            "unavailable"
        );
        item.browser_safe_not_applicable = Some("permission proof only".into());
        assert_eq!(
            run(
                root,
                &item,
                "forms/fixture/main.conduit",
                None,
                &Preparation::Ready,
                &opts,
            )
            .status,
            "not_applicable"
        );
        item.browser_safe = Some(super::super::BrowserOracle {
            spec: "proof/browser/fixture.spec.mjs".into(),
            case: "fixture".into(),
        });
        assert_eq!(
            run(
                root,
                &item,
                "forms/fixture/main.conduit",
                None,
                &Preparation::Ready,
                &opts,
            )
            .status,
            "refused"
        );
    }

    #[test]
    fn exact_plan_and_play_evidence_is_required() {
        let output = Output {
            status: success_status(),
            stdout: b"CONDUIT_FORM_EVIDENCE={\"plan_id\":\"plan/1\",\"play_id\":\"play/1\"}\n"
                .to_vec(),
            stderr: Vec::new(),
        };
        let evidence = evidence(&output).unwrap();
        assert_eq!(evidence.plan_id, "plan/1");
        assert_eq!(evidence.play_id, "play/1");
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

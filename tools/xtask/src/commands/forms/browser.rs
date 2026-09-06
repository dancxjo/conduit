//! Fresh-process execution of inventory-declared browser-safe Form proofs.

use super::{
    catalogs, check_one, composition, deterministic::bounded_reason, load_inventory, result,
    reusable, FormProofResult, InventoryForm, Report, REPORT_SCHEMA,
};
use crate::cli::GlobalOpts;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[path = "browser/batch.rs"]
mod batch;
use batch::BatchRequest;

pub(super) enum Preparation {
    Ready(PathBuf),
    Unavailable(String),
    Failed(String),
}

pub(super) fn build_report(root: &Path, opts: &GlobalOpts) -> Result<Report, String> {
    let inventory = load_inventory(root)?;
    let catalogs = catalogs()?;
    let preparation = prepare(root, &inventory.forms, opts);
    let mut checks = BTreeMap::new();
    let mut browser_results = BTreeMap::new();
    let mut pending = Vec::new();
    for form in &inventory.forms {
        let source_path = format!("forms/{}/main.conduit", form.slug);
        let started = Instant::now();
        match check_one(root, &source_path, &form.entry, &catalogs) {
            Ok((source_id, checked_id)) => {
                let identities = Some((source_id.clone(), checked_id.clone()));
                checks.insert(
                    form.slug.clone(),
                    result(
                        form,
                        &source_path,
                        started.elapsed().as_millis(),
                        "passed",
                        "canonical source parsed and checked through the standard semantic catalog",
                        identities.clone(),
                        "check",
                    ),
                );
                if let (Some(oracle), None, Preparation::Ready(_)) = (
                    &form.browser_safe,
                    &form.browser_safe_not_applicable,
                    &preparation,
                ) {
                    if !opts.dry_run {
                        pending.push(BatchRequest {
                            form,
                            path: source_path,
                            identities,
                            oracle,
                        });
                        continue;
                    }
                }
                browser_results.insert(
                    form.slug.clone(),
                    run(root, form, &source_path, identities, &preparation, opts),
                );
            }
            Err(reason) => {
                checks.insert(
                    form.slug.clone(),
                    result(
                        form,
                        &source_path,
                        started.elapsed().as_millis(),
                        "failed",
                        &reason,
                        None,
                        "check",
                    ),
                );
            }
        }
    }
    let process_starts = usize::from(!pending.is_empty());
    let process_starts_avoided = pending.len().saturating_sub(process_starts);
    if let Preparation::Ready(playwright) = &preparation {
        for proof in batch::execute(root, playwright, pending) {
            browser_results.insert(proof.slug.clone(), proof);
        }
    }
    let mut results = Vec::with_capacity(inventory.forms.len() * 2);
    for form in &inventory.forms {
        let source_path = format!("forms/{}/main.conduit", form.slug);
        if let Some(check) = checks.remove(&form.slug) {
            results.push(check);
        }
        if let Some(proof) = browser_results.remove(&form.slug) {
            results.push(proof);
        }
        results.extend(reusable::check_all(root, form, &source_path, &catalogs));
        results.extend(composition::check_all(root, form, &source_path, &catalogs));
    }
    Ok(Report {
        schema: REPORT_SCHEMA,
        inventory_schema: inventory.schema,
        proof_process_starts: process_starts,
        proof_process_starts_avoided: process_starts_avoided,
        results,
    })
}

pub(super) fn prepare(root: &Path, forms: &[InventoryForm], opts: &GlobalOpts) -> Preparation {
    if !forms.iter().any(|form| form.browser_safe.is_some()) {
        return Preparation::Ready(batch::local_playwright(root));
    }
    if opts.dry_run {
        return Preparation::Unavailable(
            "dry run planned: build the browser runtime before isolated Chromium proofs".into(),
        );
    }
    let playwright = batch::local_playwright(root);
    if !playwright.is_file() {
        return Preparation::Unavailable(format!(
            "repository Playwright binary is absent: {}",
            playwright.display()
        ));
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
        Ok(output) if output.status.success() => Preparation::Ready(playwright),
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
    let mut proof = if !matches!(
        (&form.browser_safe, &form.browser_safe_not_applicable),
        (Some(_), None)
    ) {
        availability(form, path, identities)
    } else {
        let oracle = form
            .browser_safe
            .as_ref()
            .expect("matched browser-safe oracle");
        match preparation {
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
            Preparation::Ready(_) if opts.dry_run => result(
                form,
                path,
                0,
                "unavailable",
                "dry run planned: isolated Playwright Chromium proof",
                identities,
                "browser-safe",
            ),
            Preparation::Ready(playwright) => batch::execute(
                root,
                playwright,
                vec![BatchRequest {
                    form,
                    path: path.to_owned(),
                    identities,
                    oracle,
                }],
            )
            .pop()
            .expect("one batch request produces one result"),
        }
    };
    proof.environment_profile = "playwright/chromium-1.62.0-worker1-retry0";
    proof
}

pub(super) fn availability(
    form: &InventoryForm,
    path: &str,
    identities: Option<(String, String)>,
) -> FormProofResult {
    let mut proof = match (&form.browser_safe, &form.browser_safe_not_applicable) {
        (Some(_), None) => result(
            form,
            path,
            0,
            "unavailable",
            "declared browser-safe oracle is available through Playwright Chromium",
            identities,
            "browser-safe",
        ),
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
    };
    proof.environment_profile = "playwright/chromium-1.62.0-worker1-retry0";
    proof
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
        let root = Path::new(".");
        let opts = GlobalOpts::default();
        let mut item = form();
        assert_eq!(
            run(
                root,
                &item,
                "forms/fixture/main.conduit",
                None,
                &Preparation::Ready(PathBuf::from("node_modules/.bin/playwright")),
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
                &Preparation::Ready(PathBuf::from("node_modules/.bin/playwright")),
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
                &Preparation::Ready(PathBuf::from("node_modules/.bin/playwright")),
                &opts,
            )
            .status,
            "refused"
        );
    }

    #[test]
    fn missing_repository_playwright_is_unavailable_without_network_fallback() {
        let mut item = form();
        item.browser_safe = Some(super::super::BrowserOracle {
            spec: "proof/browser/fixture.spec.mjs".into(),
            case: "fixture".into(),
        });
        let preparation = prepare(
            Path::new("/conduit-fixture-without-node-modules"),
            &[item],
            &GlobalOpts::default(),
        );
        match preparation {
            Preparation::Unavailable(reason) => {
                assert!(reason.contains("repository Playwright binary is absent"));
                assert!(!reason.contains("npx"));
            }
            _ => panic!("missing admitted tooling must be unavailable"),
        }
    }
}

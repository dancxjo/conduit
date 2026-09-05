//! Side-effect-free projection of checked Form and declared proof availability.

use super::{
    browser, catalogs, check_one, combined, composition, deterministic, load_inventory, result,
    reusable, Report, REPORT_SCHEMA,
};
use std::path::Path;
use std::time::Instant;

pub(super) fn build(root: &Path) -> Result<Report, String> {
    let inventory = load_inventory(root)?;
    let catalogs = catalogs()?;
    let mut results = Vec::with_capacity(inventory.forms.len() * 3);
    for form in &inventory.forms {
        let source_path = format!("forms/{}/main.conduit", form.slug);
        let started = Instant::now();
        match check_one(root, &source_path, &form.entry, &catalogs) {
            Ok((source_id, checked_id)) => {
                let identities = Some((source_id.clone(), checked_id.clone()));
                results.push(result(
                    form,
                    &source_path,
                    started.elapsed().as_millis(),
                    "passed",
                    "canonical source parsed and checked through the standard semantic catalog",
                    identities.clone(),
                    "check",
                ));
                results.push(deterministic::availability(
                    form,
                    &source_path,
                    identities.clone(),
                ));
                results.push(browser::availability(form, &source_path, identities));
            }
            Err(reason) => {
                results.push(result(
                    form,
                    &source_path,
                    started.elapsed().as_millis(),
                    "failed",
                    &reason,
                    None,
                    "check",
                ));
                for mode in ["deterministic", "browser-safe"] {
                    results.push(result(
                        form,
                        &source_path,
                        0,
                        "refused",
                        "canonical source failed checking, so execution availability is inapplicable",
                        None,
                        mode,
                    ));
                }
            }
        }
        results.extend(reusable::check_all(root, form, &source_path, &catalogs));
        results.extend(composition::check_all(root, form, &source_path, &catalogs));
        results.extend(reusable::deterministic_all(
            root,
            form,
            &source_path,
            &catalogs,
            false,
            &crate::cli::GlobalOpts::default(),
        ));
    }
    results.extend(combined::results(
        root,
        &inventory,
        &catalogs,
        false,
        &crate::cli::GlobalOpts::default(),
    ));
    Ok(Report {
        schema: REPORT_SCHEMA,
        inventory_schema: inventory.schema,
        proof_process_starts: 0,
        proof_process_starts_avoided: 0,
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_report_has_every_mode_without_execution_claims() {
        let root = crate::workspace::workspace_root().unwrap();
        let report = build(&root).unwrap();
        assert_eq!(report.results.len(), 48 * 3 + 10 * 3 + 3);
        for slug in report
            .results
            .iter()
            .filter(|result| {
                !matches!(
                    result.proof_mode,
                    "reusable-check"
                        | "composition-check"
                        | "reusable-deterministic"
                        | "combined-deterministic"
                )
            })
            .map(|result| &result.slug)
            .collect::<std::collections::BTreeSet<_>>()
        {
            let modes = report
                .results
                .iter()
                .filter(|result| {
                    &result.slug == slug
                        && !matches!(
                            result.proof_mode,
                            "reusable-check"
                                | "composition-check"
                                | "reusable-deterministic"
                                | "combined-deterministic"
                        )
                })
                .map(|result| result.proof_mode)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(modes, ["browser-safe", "check", "deterministic"].into());
        }
        assert!(report.results.iter().all(|result| {
            matches!(
                result.proof_mode,
                "check" | "reusable-check" | "composition-check"
            ) || (result.plan_id.is_none()
                && result.play_id.is_none()
                && result.duration_millis == 0
                && result.status != "passed")
        }));
    }
}

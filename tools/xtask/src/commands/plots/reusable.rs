//! Independent semantic checks for reusable Plots embedded in canonical sources.

use super::{check_one, deterministic, result, InventoryPlot, PlotProofResult};
use crate::cli::GlobalOpts;
use std::path::Path;
use std::time::Instant;

pub(super) fn check_all(
    root: &Path,
    plot: &InventoryPlot,
    path: &str,
    catalogs: &(conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog),
) -> Vec<PlotProofResult> {
    plot.reusable_entries
        .iter()
        .map(|reusable| {
            let started = Instant::now();
            let checked = check_one(root, path, &reusable.entry, catalogs);
            let (status, reason, identities) = match checked {
                Ok(identities) => (
                    "passed",
                    "reusable Plot independently parsed and checked through the standard semantic catalog"
                        .to_string(),
                    Some(identities),
                ),
                Err(reason) => ("failed", reason, None),
            };
            let mut proof = result(
                plot,
                path,
                started.elapsed().as_millis(),
                status,
                &reason,
                identities,
                "reusable-check",
            );
            proof.title = reusable.title.clone();
            proof.plot_entry = reusable.entry.clone();
            proof
        })
        .collect()
}

pub(super) fn deterministic_all(
    root: &Path,
    plot: &InventoryPlot,
    path: &str,
    catalogs: &(conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog),
    execute: bool,
    opts: &GlobalOpts,
) -> Vec<PlotProofResult> {
    plot.reusable_entries
        .iter()
        .map(|reusable| match check_one(root, path, &reusable.entry, catalogs) {
            Ok(identities) if execute => {
                deterministic::run_reusable(root, plot, reusable, path, Some(identities), opts)
            }
            Ok(identities) => {
                deterministic::reusable_availability(plot, reusable, path, Some(identities))
            }
            Err(reason) => {
                let mut proof = result(
                    plot,
                    path,
                    0,
                    "refused",
                    &format!(
                        "reusable Plot failed checking, so deterministic execution is inapplicable: {reason}"
                    ),
                    None,
                    "reusable-deterministic",
                );
                proof.title = reusable.title.clone();
                proof.plot_entry = reusable.entry.clone();
                proof
            }
        })
        .collect()
}

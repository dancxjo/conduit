//! Checked evidence that a reviewed reusable Plot is consumed through its front.

use super::{result, InventoryPlot, PlotProofResult};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Instant;

pub(super) fn check_all(
    root: &Path,
    plot: &InventoryPlot,
    path: &str,
    catalogs: &(conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog),
) -> Vec<PlotProofResult> {
    let document = fs::read_to_string(root.join(path))
        .map_err(|error| format!("{path}: {error}"))
        .and_then(|source| {
            let syntax = conduit_plot::parse_syntax_document(&source);
            if let Some(diagnostic) = syntax.diagnostics.first() {
                return Err(format!(
                    "{path}: {}: {}",
                    diagnostic.code, diagnostic.message
                ));
            }
            conduit_plot::check_syntax_document(&syntax, &catalogs.0)
                .map_err(|error| format!("{path}: {}: {}", error.code, error.message))
        });

    plot.reusable_entries
        .iter()
        .map(|reusable| {
            let started = Instant::now();
            let subject = document.as_ref().map_err(Clone::clone).and_then(|checked| {
                let reusable_plot = checked
                    .plots
                    .iter()
                    .find(|candidate| candidate.name == reusable.entry)
                    .ok_or_else(|| {
                        format!("declared reusable Plot '{}' is absent", reusable.entry)
                    })?;
                Ok((
                    checked.source_document_id.as_str().to_string(),
                    reusable_plot.checked_plot_id.as_str().to_string(),
                ))
            });
            let Some(oracle) = &reusable.composition else {
                return proof(
                    plot,
                    path,
                    reusable,
                    started.elapsed().as_millis(),
                    (
                        "unavailable",
                        "no reviewed parent composition is declared for this reusable Plot",
                    ),
                    subject.ok(),
                    None,
                );
            };
            let evidence = document.as_ref().map_err(Clone::clone).and_then(|checked| {
                let parent = checked
                    .plots
                    .iter()
                    .find(|candidate| candidate.name == oracle.parent)
                    .ok_or_else(|| format!("declared parent Plot '{}' is absent", oracle.parent))?;
                let actual = parent
                    .gears
                    .iter()
                    .filter(|gear| gear.kind == reusable.entry)
                    .filter_map(|gear| gear.name.clone())
                    .chain(
                        parent
                            .pools
                            .iter()
                            .filter(|pool| pool.member_plot == reusable.entry)
                            .map(|pool| pool.name.clone()),
                    )
                    .collect::<BTreeSet<_>>();
                let expected = oracle.occurrences.iter().cloned().collect::<BTreeSet<_>>();
                if actual != expected {
                    return Err(format!(
                        "parent '{}' uses '{}' at {actual:?}, expected {expected:?}",
                        oracle.parent, reusable.entry
                    ));
                }
                Ok(parent.checked_plot_id.as_str().to_string())
            });
            match evidence {
                Ok(parent_id) => proof(
                    plot,
                    path,
                    reusable,
                    started.elapsed().as_millis(),
                    (
                        "passed",
                        "checked parent consumes the reusable Plot through its exact front",
                    ),
                    subject.ok(),
                    Some(parent_id),
                ),
                Err(reason) => proof(
                    plot,
                    path,
                    reusable,
                    started.elapsed().as_millis(),
                    ("failed", &reason),
                    subject.ok(),
                    None,
                ),
            }
        })
        .collect()
}

fn proof(
    plot: &InventoryPlot,
    path: &str,
    reusable: &super::ReusablePlot,
    duration: u128,
    outcome: (&str, &str),
    identities: Option<(String, String)>,
    parent_id: Option<String>,
) -> PlotProofResult {
    let mut proof = result(
        plot,
        path,
        duration,
        outcome.0,
        outcome.1,
        identities,
        "composition-check",
    );
    proof.title = reusable.title.clone();
    proof.plot_entry = reusable.entry.clone();
    if let (Some(oracle), Some(parent_id)) = (&reusable.composition, parent_id) {
        proof.composition_root_entry = Some(oracle.parent.clone());
        proof.composition_root_checked_plot_id = Some(parent_id);
        proof.gear_occurrences = oracle.occurrences.clone();
    }
    proof
}

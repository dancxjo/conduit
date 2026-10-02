//! One-oracle execution and per-Plot projection for declared Body workloads.

use super::{
    check_one, deterministic, result, CombinedWorkload, Inventory, InventoryPlot, PlotProofResult,
    INVENTORY_PATH,
};
use crate::cli::GlobalOpts;
use std::path::Path;

type Catalogs = (conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog);

pub(super) fn results(
    root: &Path,
    inventory: &Inventory,
    catalogs: &Catalogs,
    execute: bool,
    opts: &GlobalOpts,
) -> Vec<PlotProofResult> {
    inventory
        .combined_workloads
        .iter()
        .flat_map(|workload| workload_results(root, inventory, catalogs, workload, execute, opts))
        .collect()
}

fn workload_results(
    root: &Path,
    inventory: &Inventory,
    catalogs: &Catalogs,
    workload: &CombinedWorkload,
    execute: bool,
    opts: &GlobalOpts,
) -> Vec<PlotProofResult> {
    let checked = workload
        .entries
        .iter()
        .map(|entry| {
            let plot = inventory
                .plots
                .iter()
                .find(|plot| plot.slug == entry.slug && plot.entry == entry.entry)
                .expect("validated combined workload entry");
            let path = format!("plots/{}/main.conduit", plot.slug);
            check_one(root, &path, &plot.entry, catalogs).map(|identities| (plot, path, identities))
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(checked) = checked else {
        let reason = checked.unwrap_err();
        return workload
            .entries
            .iter()
            .map(|entry| {
                let plot = find_plot(inventory, &entry.slug);
                combined_result(
                    workload,
                    plot,
                    &format!("plots/{}/main.conduit", plot.slug),
                    None,
                    "refused",
                    &format!("combined workload checking failed: {reason}"),
                )
            })
            .collect();
    };
    if !execute || opts.dry_run {
        return checked
            .into_iter()
            .map(|(plot, path, identities)| {
                combined_result(
                    workload,
                    plot,
                    &path,
                    Some(identities),
                    "unavailable",
                    "declared combined deterministic oracle is available through cargo xtask check plots run --deterministic",
                )
            })
            .collect();
    }

    let (first_plot, first_path, first_identities) = &checked[0];
    let mut proof = deterministic::execute(
        root,
        first_plot,
        first_path,
        Some(first_identities.clone()),
        &workload.deterministic,
        opts,
        "combined-deterministic",
    );
    if proof.status == "passed" && proof.workload_revision != Some(workload.workload_revision) {
        proof.status = "failed".into();
        proof.reason = format!(
            "combined workload '{}' expected revision {}, oracle reported {:?}",
            workload.slug, workload.workload_revision, proof.workload_revision
        );
    }
    let artifacts = std::iter::once(INVENTORY_PATH.into())
        .chain(checked.iter().map(|(_, path, _)| path.clone()))
        .collect::<Vec<_>>();
    checked
        .into_iter()
        .map(|(plot, path, identities)| {
            let mut projected = proof.clone();
            projected.slug = plot.slug.clone();
            projected.title = plot.title.clone();
            projected.source_path = path;
            projected.plot_entry = plot.entry.clone();
            projected.source_document_id = Some(identities.0);
            projected.checked_plot_id = Some(identities.1);
            projected.workload_slug = Some(workload.slug.clone());
            projected.workload_title = Some(workload.title.clone());
            projected.evidence_artifacts = artifacts.clone();
            projected
        })
        .collect()
}

fn find_plot<'a>(inventory: &'a Inventory, slug: &str) -> &'a InventoryPlot {
    inventory
        .plots
        .iter()
        .find(|plot| plot.slug == slug)
        .expect("validated combined workload entry")
}

fn combined_result(
    workload: &CombinedWorkload,
    plot: &InventoryPlot,
    path: &str,
    identities: Option<(String, String)>,
    status: &str,
    reason: &str,
) -> PlotProofResult {
    let mut proof = result(
        plot,
        path,
        0,
        status,
        reason,
        identities,
        "combined-deterministic",
    );
    proof.workload_slug = Some(workload.slug.clone());
    proof.workload_title = Some(workload.title.clone());
    proof
}

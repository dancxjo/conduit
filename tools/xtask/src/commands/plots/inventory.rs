//! Finite reviewed plot inventory loading and structural validation.

use super::{CombinedWorkload, Inventory, InventoryPlot, INVENTORY_PATH, INVENTORY_SCHEMA};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) fn load_inventory(root: &Path) -> Result<Inventory, String> {
    let bytes = fs::read_to_string(root.join(INVENTORY_PATH)).map_err(|error| error.to_string())?;
    let inventory: Inventory = toml::from_str(&bytes).map_err(|error| error.to_string())?;
    let reviewed_subjects = inventory
        .plots
        .iter()
        .map(|plot| 1 + plot.reusable_entries.len())
        .sum::<usize>();
    if inventory.schema != INVENTORY_SCHEMA
        || inventory.plots.is_empty()
        || reviewed_subjects > inventory.maximum_plots
        || inventory.combined_workloads.is_empty()
        || inventory.maximum_combined_workloads == 0
        || inventory.combined_workloads.len() > inventory.maximum_combined_workloads
    {
        return Err("reviewed plot inventory violates its schema or finite bound".into());
    }
    let mut workload_slugs = BTreeSet::new();
    for workload in &inventory.combined_workloads {
        if !combined_workload_is_valid(workload, &inventory.plots)
            || !workload_slugs.insert(workload.slug.as_str())
        {
            return Err(format!(
                "reviewed combined workload '{}' is invalid or duplicate",
                workload.slug
            ));
        }
    }
    let mut slugs = BTreeSet::new();
    let mut browser_oracles = BTreeSet::new();
    for plot in &inventory.plots {
        if plot.slug.is_empty()
            || plot.title.is_empty()
            || plot.entry.is_empty()
            || !slugs.insert(&plot.slug)
        {
            return Err("reviewed plot inventory contains an empty or duplicate identity".into());
        }
        if !reusable_entries_are_valid(plot) {
            return Err(format!(
                "reviewed plot '{}' contains an invalid or duplicate reusable identity",
                plot.slug
            ));
        }
        if let Some(oracle) = &plot.browser_safe {
            if oracle.case.is_empty()
                || oracle.case.len() > 160
                || !oracle.spec.starts_with("proof/browser/")
                || !oracle.spec.ends_with(".spec.mjs")
                || oracle.spec.contains("..")
                || !root.join(&oracle.spec).is_file()
                || !browser_oracles.insert((&oracle.spec, &oracle.case))
            {
                return Err(format!(
                    "reviewed plot '{}' has an invalid or duplicate browser-safe oracle",
                    plot.slug
                ));
            }
        }
    }
    let declared: BTreeSet<String> = slugs.into_iter().cloned().collect();
    let mut present = BTreeSet::new();
    for entry in fs::read_dir(root.join("plots")).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
            && entry.path().join("main.conduit").is_file()
        {
            present.insert(
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "non-UTF-8 Plot path")?,
            );
        }
    }
    if declared != present {
        return Err(format!(
            "reviewed plot inventory mismatch: declared {declared:?}, canonical sources {present:?}"
        ));
    }
    Ok(inventory)
}

fn combined_workload_is_valid(workload: &CombinedWorkload, plots: &[InventoryPlot]) -> bool {
    let mut entries = BTreeSet::new();
    !workload.slug.is_empty()
        && !workload.title.is_empty()
        && workload.entries.len() >= 2
        && workload.entries.len() <= conduit_body::MAX_BODY_PLOTS
        && workload.deterministic.plan_play_evidence
        && workload.deterministic.workload_revision_evidence
        && workload.entries.iter().all(|entry| {
            entries.insert((entry.slug.as_str(), entry.entry.as_str()))
                && plots
                    .iter()
                    .any(|plot| plot.slug == entry.slug && plot.entry == entry.entry)
        })
}

pub(super) fn reusable_entries_are_valid(plot: &InventoryPlot) -> bool {
    let mut entries = BTreeSet::from([plot.entry.as_str()]);
    plot.reusable_entries.iter().all(|reusable| {
        let composition_valid = reusable.composition.as_ref().is_none_or(|oracle| {
            let occurrences = oracle
                .occurrences
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            !oracle.parent.is_empty()
                && oracle.parent != reusable.entry
                && !oracle.occurrences.is_empty()
                && occurrences.len() == oracle.occurrences.len()
                && occurrences.iter().all(|occurrence| !occurrence.is_empty())
        });
        !reusable.entry.is_empty()
            && !reusable.title.is_empty()
            && entries.insert(reusable.entry.as_str())
            && composition_valid
    })
}

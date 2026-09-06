//! One-oracle execution and per-Form projection for declared Body workloads.

use super::{
    check_one, deterministic, result, CombinedWorkload, FormProofResult, Inventory, InventoryForm,
    INVENTORY_PATH,
};
use crate::cli::GlobalOpts;
use std::path::Path;

type Catalogs = (conduit_form::StartupCatalog, conduit_form::ProfileCatalog);

pub(super) fn results(
    root: &Path,
    inventory: &Inventory,
    catalogs: &Catalogs,
    execute: bool,
    opts: &GlobalOpts,
) -> Vec<FormProofResult> {
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
) -> Vec<FormProofResult> {
    let checked = workload
        .entries
        .iter()
        .map(|entry| {
            let form = inventory
                .forms
                .iter()
                .find(|form| form.slug == entry.slug && form.entry == entry.entry)
                .expect("validated combined workload entry");
            let path = format!("forms/{}/main.conduit", form.slug);
            check_one(root, &path, &form.entry, catalogs).map(|identities| (form, path, identities))
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(checked) = checked else {
        let reason = checked.unwrap_err();
        return workload
            .entries
            .iter()
            .map(|entry| {
                let form = find_form(inventory, &entry.slug);
                combined_result(
                    workload,
                    form,
                    &format!("forms/{}/main.conduit", form.slug),
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
            .map(|(form, path, identities)| {
                combined_result(
                    workload,
                    form,
                    &path,
                    Some(identities),
                    "unavailable",
                    "declared combined deterministic oracle is available through cargo xtask forms run --deterministic",
                )
            })
            .collect();
    }

    let (first_form, first_path, first_identities) = &checked[0];
    let mut proof = deterministic::execute(
        root,
        first_form,
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
        .map(|(form, path, identities)| {
            let mut projected = proof.clone();
            projected.slug = form.slug.clone();
            projected.title = form.title.clone();
            projected.source_path = path;
            projected.form_entry = form.entry.clone();
            projected.source_document_id = Some(identities.0);
            projected.checked_form_id = Some(identities.1);
            projected.workload_slug = Some(workload.slug.clone());
            projected.workload_title = Some(workload.title.clone());
            projected.evidence_artifacts = artifacts.clone();
            projected
        })
        .collect()
}

fn find_form<'a>(inventory: &'a Inventory, slug: &str) -> &'a InventoryForm {
    inventory
        .forms
        .iter()
        .find(|form| form.slug == slug)
        .expect("validated combined workload entry")
}

fn combined_result(
    workload: &CombinedWorkload,
    form: &InventoryForm,
    path: &str,
    identities: Option<(String, String)>,
    status: &str,
    reason: &str,
) -> FormProofResult {
    let mut proof = result(
        form,
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

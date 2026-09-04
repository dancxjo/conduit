//! Independent semantic checks for reusable Forms embedded in canonical sources.

use super::{check_one, result, FormProofResult, InventoryForm};
use std::path::Path;
use std::time::Instant;

pub(super) fn check_all(
    root: &Path,
    form: &InventoryForm,
    path: &str,
    catalogs: &(conduit_form::StartupCatalog, conduit_form::ProfileCatalog),
) -> Vec<FormProofResult> {
    form.reusable_entries
        .iter()
        .map(|reusable| {
            let started = Instant::now();
            let checked = check_one(root, path, &reusable.entry, catalogs);
            let (status, reason, identities) = match checked {
                Ok(identities) => (
                    "passed",
                    "reusable Form independently parsed and checked through the standard semantic catalog"
                        .to_string(),
                    Some(identities),
                ),
                Err(reason) => ("failed", reason, None),
            };
            let mut proof = result(
                form,
                path,
                started.elapsed().as_millis(),
                status,
                &reason,
                identities,
                "reusable-check",
            );
            proof.title = reusable.title.clone();
            proof.form_entry = reusable.entry.clone();
            proof
        })
        .collect()
}

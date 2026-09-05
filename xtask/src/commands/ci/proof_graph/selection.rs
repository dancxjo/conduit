use super::spec::{ProofSpec, Selection};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub(super) struct ImpactSelection {
    pub(super) workspace_shards: BTreeMap<String, bool>,
    pub(super) full_fallback: bool,
    pub(super) pages_products_required: bool,
    pub(super) pages_product_proofs: Vec<String>,
    pub(super) esp32_required: bool,
    pub(super) esp32_targets: Vec<String>,
}

pub(super) fn load(
    path: Option<&Path>,
) -> Result<Option<ImpactSelection>, Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(None);
    };
    // Applicability is an optimization boundary, not admission authority. An
    // expired or malformed retained plan therefore selects the conservative
    // complete registry rather than preventing fresh proof execution.
    let Ok(bytes) = fs::read(path) else {
        return Ok(None);
    };
    Ok(serde_json::from_slice(&bytes).ok())
}

pub(super) fn is_selected(spec: &ProofSpec, selected: Option<&ImpactSelection>) -> bool {
    let Some(selected) = selected else {
        return true;
    };
    if selected.full_fallback {
        return true;
    }
    match spec.selection {
        Selection::WorkspaceShard(shard) => selected
            .workspace_shards
            .get(shard)
            .copied()
            .unwrap_or(true),
        Selection::PagesProducts => selected.pages_products_required,
        Selection::PagesProductProof(id) => selected
            .pages_product_proofs
            .iter()
            .any(|candidate| candidate == id),
        Selection::Esp32Target(target) => {
            selected.esp32_required
                && selected
                    .esp32_targets
                    .iter()
                    .any(|candidate| candidate == target)
        }
    }
}

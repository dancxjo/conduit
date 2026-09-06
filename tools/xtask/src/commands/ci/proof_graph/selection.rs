use super::spec::{ProofSpec, Selection};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub(super) struct ImpactSelection {
    pub(super) ci_controller_proofs: Vec<String>,
    pub(super) workspace_shards: BTreeMap<String, bool>,
    pub(super) full_fallback: bool,
    #[serde(default)]
    pub(super) shared_compile_packages: Vec<String>,
    pub(super) pages_products_required: bool,
    pub(super) pages_product_proofs: Vec<String>,
    pub(super) esp32_required: bool,
    pub(super) esp32_targets: Vec<String>,
    pub(super) conduitos_required: bool,
    pub(super) conduitos_x86_proofs: Vec<String>,
    pub(super) conduitos_architectures: Vec<String>,
    pub(super) conduitos_aarch64_product_required: bool,
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
    if matches!(spec.selection, Selection::SharedCompile) {
        return !selected.shared_compile_packages.is_empty();
    }
    if selected.full_fallback {
        return true;
    }
    match spec.selection {
        Selection::CiController => !selected.ci_controller_proofs.is_empty(),
        Selection::SharedCompile => unreachable!("shared compile handled before fallback"),
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
        Selection::Esp32Required => selected.esp32_required,
        Selection::Esp32Target(target) => {
            selected.esp32_required
                && selected
                    .esp32_targets
                    .iter()
                    .any(|candidate| candidate == target)
        }
        Selection::ConduitosRequired => selected.conduitos_required,
        Selection::ConduitosX86(proof) => selected
            .conduitos_x86_proofs
            .iter()
            .any(|candidate| candidate == proof),
        Selection::ConduitosArchitecture(architecture) => selected
            .conduitos_architectures
            .iter()
            .any(|candidate| candidate == architecture),
        Selection::ConduitosAarch64Product => selected.conduitos_aarch64_product_required,
    }
}

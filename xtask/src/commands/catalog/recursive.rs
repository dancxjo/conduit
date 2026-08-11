use super::{CatalogError, InstalledImplementation};

#[derive(Clone)]
pub(super) struct Coverage {
    pub(super) host_profile: String,
    pub(super) kind_id: String,
    pub(super) contract_revision: String,
    pub(super) realization_id: String,
    pub(super) leaves: Vec<InstalledImplementation>,
}

pub(super) fn derive() -> Result<Vec<Coverage>, CatalogError> {
    let proof = patchbay_model::patchbay_presenter_plans()
        .map_err(|error| CatalogError::new("patchbay-recursive-profile-invalid", error))?;
    let placements = proof
        .recursive
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    Ok(proof
        .recursive
        .realization_backs
        .iter()
        .map(|back| Coverage {
            host_profile: proof.recursive_host.profile.as_str().to_owned(),
            kind_id: back.kind_id.as_str().to_owned(),
            contract_revision: back.kind_contract_revision.as_str().to_owned(),
            realization_id: identity(back),
            leaves: placements
                .iter()
                .filter(|placement| {
                    placement
                        .gear_id
                        .as_str()
                        .starts_with(&format!("{}/", back.invocation_path))
                })
                .map(|placement| InstalledImplementation {
                    implementation_id: placement.implementation_id.as_str().to_owned(),
                    artifact_id: placement.artifact_id.as_str().to_owned(),
                    execution_profile_id: placement.execution_profile_id.as_str().to_owned(),
                    host_operation_families: placement
                        .host_operations
                        .iter()
                        .map(|operation| operation.contract_id.as_str().to_owned())
                        .collect(),
                    resource_families: placement
                        .resources
                        .iter()
                        .map(|resource| resource.class_id.as_str().to_owned())
                        .collect(),
                })
                .collect(),
        })
        .collect())
}

fn identity(back: &conduit_core::RealizationBack) -> String {
    format!(
        "canonical-back:{}:{}",
        back.source_document_id.as_str(),
        back.checked_form_id.as_str()
    )
}

use super::{CatalogError, InstalledImplementation};

#[derive(Clone)]
pub(crate) struct Coverage {
    pub(crate) host_profile: String,
    pub(crate) kind_id: String,
    pub(crate) contract_revision: String,
    pub(super) realization_id: String,
    pub(super) leaves: Vec<InstalledImplementation>,
}

pub(crate) fn derive() -> Result<Vec<Coverage>, CatalogError> {
    let proof = patchbay_model::patchbay_presenter_plans()
        .map_err(|error| CatalogError::new("patchbay-recursive-profile-invalid", error))?;
    let conduitos = conduitos::presentation_nucleus::prepare(
        "catalog-conduitos-reference",
        "catalog-static-not-a-boot",
    )
    .map_err(|error| CatalogError::new("conduitos-recursive-profile-invalid", error.as_str()))?;
    let mut entries = coverage(proof.recursive_host.profile.as_str(), &proof.recursive);
    entries.extend(coverage(
        conduitos.advertisement.profile.as_str(),
        &conduitos.plan,
    ));
    Ok(entries)
}

fn coverage(host_profile: &str, plan: &conduit_core::Plan) -> Vec<Coverage> {
    let placements = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    plan.realization_backs
        .iter()
        .map(|back| Coverage {
            host_profile: host_profile.to_owned(),
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
        .collect()
}

fn identity(back: &conduit_core::RealizationBack) -> String {
    format!(
        "canonical-back:{}:{}",
        back.source_document_id.as_str(),
        back.checked_form_id.as_str()
    )
}

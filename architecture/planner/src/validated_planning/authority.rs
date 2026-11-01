//! Exact admitted authority requirements retain the ordinary grant resolver.
use super::*;
pub(super) fn bind(
    host: &HostAdvertisement,
    capability: &conduit_core::CapabilityOffer,
    authority_grants: &[conduit_core::AuthorityGrant],
) -> Result<Vec<AuthorityBinding>, PlannerError> {
    let mut authority_bindings = Vec::with_capacity(capability.authority_requirements.len());
    for requirement in &capability.authority_requirements {
        let grant_index = resolve_authority_grant_index(
            host.into(),
            capability.into(),
            requirement.into(),
            conduit_core::AuthorityGrantRefs::Owned(authority_grants),
        )?;
        let grant = &authority_grants[grant_index];
        authority_bindings.push(AuthorityBinding {
            grant_id: grant.grant_id.clone(),
            contract_id: grant.contract_id.clone(),
            host_call_contract_id: grant.host_call_contract_id.clone(),
            subject_kind: grant.subject_kind.clone(),
            host_id: grant.host_id.clone(),
            boot_id: grant.boot_id.clone(),
            capability_id: grant.capability_id.clone(),
        });
    }
    authority_bindings.sort_by(|a, b| compare_authority_bindings_ref(a.into(), b.into()));

    Ok(authority_bindings)
}

use crate::PlannerError;
use conduit_core::{
    CapabilityOffer, HostAdvertisement, ProtectedResourceAccess, ProtectedResourceBinding,
    ProtectedResourceCommitPolicy, ProtectedResourceGrant, ResourceHandleId, ResourceRequirement,
};
use conduit_form::CheckedOperation;
use std::collections::BTreeSet;

pub(crate) fn validate_protected_resource_grants(
    grants: &[ProtectedResourceGrant],
) -> Result<(), PlannerError> {
    if grants.iter().any(|grant| {
        grant.role_id.as_str().is_empty()
            || grant.handle_id.as_str().is_empty()
            || grant.operation_id.as_str().is_empty()
            || grant.host_id.as_str().is_empty()
            || grant.boot_id.as_str().is_empty()
            || grant.capability_id.as_str().is_empty()
            || grant.class_id.as_str().is_empty()
            || grant.maximum_bytes == 0
            || !matches!(
                (grant.access, grant.commit_policy),
                (
                    ProtectedResourceAccess::ReadExisting,
                    ProtectedResourceCommitPolicy::NotApplicable
                ) | (
                    ProtectedResourceAccess::Create,
                    ProtectedResourceCommitPolicy::CreateOnly
                ) | (
                    ProtectedResourceAccess::Replace,
                    ProtectedResourceCommitPolicy::ReplaceExisting
                )
            )
    }) {
        return Err(PlannerError::InvalidProtectedResourceGrant(
            "grants require non-empty exact identities, a positive byte bound, and a matching access/commit policy"
                .to_string(),
        ));
    }

    let unique_handles = grants
        .iter()
        .map(|grant| &grant.handle_id)
        .collect::<BTreeSet<_>>();
    if unique_handles.len() != grants.len() {
        return Err(PlannerError::InvalidProtectedResourceGrant(
            "a protected handle may be consumed by only one planned role".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn bind_protected_resource(
    requirement: &ResourceRequirement,
    grants: &[ProtectedResourceGrant],
    operation: &CheckedOperation,
    host: &HostAdvertisement,
    capability: &CapabilityOffer,
    consumed_handles: &mut BTreeSet<ResourceHandleId>,
) -> Result<Option<ProtectedResourceBinding>, PlannerError> {
    let Some(role_id) = &requirement.protected_role else {
        return Ok(None);
    };
    let mut matches = grants.iter().filter(|grant| {
        grant.role_id == *role_id
            && grant.operation_id == operation.operation_id
            && grant.host_id == host.host_id
            && grant.boot_id == host.boot_id
            && grant.capability_id == capability.capability_id
            && grant.class_id == requirement.class_id
    });
    let Some(grant) = matches.next() else {
        return Err(PlannerError::ProtectedResourceGrantMissing(format!(
            "operation '{}' role '{}' requires class '{}' on host '{}' boot '{}'",
            operation.operation_id.as_str(),
            role_id.as_str(),
            requirement.class_id.as_str(),
            host.host_id.as_str(),
            host.boot_id.as_str()
        )));
    };
    if matches.next().is_some() {
        return Err(PlannerError::ProtectedResourceGrantAmbiguous(format!(
            "multiple grants satisfy operation '{}' role '{}'",
            operation.operation_id.as_str(),
            role_id.as_str()
        )));
    }
    consumed_handles.insert(grant.handle_id.clone());
    Ok(Some(ProtectedResourceBinding {
        role_id: grant.role_id.clone(),
        handle_id: grant.handle_id.clone(),
        access: grant.access,
        maximum_bytes: grant.maximum_bytes,
        commit_policy: grant.commit_policy,
    }))
}

//! One cooperative Base/resource/authority binding per selected native call.
use crate::{
    GeneratedEmbeddedPlan, GeneratedHostCall, GenerationError as E, UnsupportedPlanFeature as U,
};
use conduit_core::AssignedIdentity;
pub(crate) fn append(
    plan: &GeneratedEmbeddedPlan,
    call: &GeneratedHostCall,
    bytes: &mut Vec<u8>,
) -> Result<(), E> {
    let unsupported = || E::Unsupported(U::AssignedNativeBinding);
    let context = plan
        .native_bindings
        .iter()
        .find(|n| n.node == call.node)
        .ok_or_else(unsupported)?;
    let Some(base) = &context.base else {
        if !context.authority.is_empty() || !context.resources.is_empty() {
            return Err(unsupported());
        }
        bytes.push(0);
        return Ok(());
    };
    let authority = selected_authority(&context.authority, call).ok_or_else(unsupported)?;
    if authority.host_id.as_str() != plan.host_id
        || authority.boot_id.as_str() != plan.boot_id
        || authority.capability_id.as_str() != context.capability_id
    {
        return Err(unsupported());
    }
    let [resource] = context.resources.as_slice() else {
        return Err(unsupported());
    };
    if resource.protected.is_some()
        || base.enforcement_class != conduit_core::BaseEnforcementClass::Cooperative
    {
        return Err(unsupported());
    }
    let mut mapped = plan.resources.iter().filter(|r| r.node == call.node);
    let index = mapped.next().ok_or_else(unsupported)?.resource;
    if mapped.next().is_some() {
        return Err(unsupported());
    }
    let binding = conduit_core::AssignedNativeBinding {
        capability: identity(&context.capability_id),
        kind_revision: identity(&context.kind_revision),
        execution_profile: identity(&context.execution_profile),
        maximum_instances: context.limits.max_active_instances,
        queue_items: context.limits.max_queue_items,
        queue_bytes: context.limits.max_queue_bytes,
        base: identity(base.base_id.as_str()),
        instance: identity(base.provider_instance_id.as_str()),
        generation: base.provider_generation,
        base_implementation: identity(base.implementation_id.as_str()),
        mechanism: identity(base.mechanism_family.as_str()),
        enforcement: 0,
        grant: identity(authority.grant_id.as_str()),
        authority: identity(authority.contract_id.as_str()),
        call: identity(authority.host_call_contract_id.as_str()),
        subject: identity(authority.subject_kind.as_str()),
        host: identity(authority.host_id.as_str()),
        boot: identity(authority.boot_id.as_str()),
        authority_capability: identity(authority.capability_id.as_str()),
        resource: index,
        pool: identity(resource.pool_id.as_str()),
        resource_class: identity(resource.class_id.as_str()),
        units: resource.units,
    };
    let prefix: &[u8; conduit_core::ASSIGNED_NATIVE_CALL_PREFIX_BYTES] =
        bytes.as_slice().try_into().map_err(|_| unsupported())?;
    let mut encoded = [0; conduit_core::ASSIGNED_NATIVE_CALL_BYTES];
    let length = conduit_core::encode_assigned_native_binding(prefix, Some(&binding), &mut encoded)
        .map_err(|_| unsupported())?;
    bytes.clear();
    bytes.extend_from_slice(&encoded[..length]);
    Ok(())
}
fn identity(text: &str) -> AssignedIdentity {
    AssignedIdentity::from_text(text)
}

/// Select the original authority for this call, not an arbitrary node grant.
/// This serializes checked facts only; it does not issue or retain a bearer.
fn selected_authority<'a>(
    authorities: &'a [conduit_core::AuthorityBinding],
    call: &GeneratedHostCall,
) -> Option<&'a conduit_core::AuthorityBinding> {
    let target = call.target_kind.as_deref()?;
    let mut matches = authorities.iter().filter(|authority| {
        authority.host_call_contract_id.as_str() == call.contract_id
            && authority.subject_kind.as_str() == target
    });
    let authority = matches.next()?;
    matches.next().is_none().then_some(authority)
}

#[cfg(test)]
#[path = "assigned_native/tests.rs"]
mod tests;

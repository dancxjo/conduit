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
    let [authority] = context.authority.as_slice() else {
        return Err(unsupported());
    };
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
    bytes.push(1);
    id(bytes, &context.capability_id);
    id(bytes, &context.kind_revision);
    id(bytes, &context.execution_profile);
    bytes.extend_from_slice(&context.limits.max_active_instances.to_le_bytes());
    bytes.extend_from_slice(&context.limits.max_queue_items.to_le_bytes());
    bytes.extend_from_slice(&context.limits.max_queue_bytes.to_le_bytes());
    id(bytes, base.base_id.as_str());
    id(bytes, base.provider_instance_id.as_str());
    bytes.extend_from_slice(&base.provider_generation.to_le_bytes());
    id(bytes, base.implementation_id.as_str());
    id(bytes, base.mechanism_family.as_str());
    bytes.push(0);
    for text in [
        authority.grant_id.as_str(),
        authority.contract_id.as_str(),
        authority.host_call_contract_id.as_str(),
        authority.subject_kind.as_str(),
        authority.host_id.as_str(),
        authority.boot_id.as_str(),
        authority.capability_id.as_str(),
    ] {
        id(bytes, text);
    }
    bytes.extend_from_slice(&index.to_le_bytes());
    id(bytes, resource.pool_id.as_str());
    id(bytes, resource.class_id.as_str());
    bytes.extend_from_slice(&resource.units.to_le_bytes());
    Ok(())
}
fn id(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&AssignedIdentity::from_text(text).0);
}

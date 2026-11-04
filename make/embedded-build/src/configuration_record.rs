//! One assigned configuration ABI; resident form requires private Make bindings.
use crate::{
    BoundResidentExpression as B, GeneratedConfigurationEntry as Entry,
    GeneratedConfigurationValue as V, GeneratedEmbeddedPlan, GenerationError as E,
};
use conduit_core::AssignedComputationRequirement;
pub(crate) fn validate_bindings(plan: &GeneratedEmbeddedPlan, resident: &[B]) -> Result<(), E> {
    if resident.len() > 32 {
        return Err(E::InvalidComputationConfiguration);
    }
    for (i, b) in resident.iter().enumerate() {
        if b.plan.as_str() != plan.plan_id
            || b.fragment.as_str() != plan.fragment_id
            || resident[..i].iter().any(|r| r.node == b.node)
        {
            return Err(E::IdentityMismatch);
        }
        let node = plan
            .nodes
            .iter()
            .find(|n| n.node == b.node)
            .ok_or(E::IdentityMismatch)?;
        if node.kind_id != b.kind.as_str()
            || node.implementation_id != crate::RESIDENT_EXPRESSION_IMPLEMENTATION
            || node.artifact_id != b.artifact.as_str()
            || node.maximum_step_fuel != b.step_fuel
        {
            return Err(E::InvalidComputationConfiguration);
        }
        let entry = plan
            .configuration
            .iter()
            .find(|c| c.node == b.node && c.key == "program")
            .ok_or(E::InvalidComputationConfiguration)?;
        let V::Text(text) = &entry.value else {
            return Err(E::InvalidComputationConfiguration);
        };
        if AssignedComputationRequirement::from_text(b.node, "program", text)
            .map_err(|_| E::InvalidComputationConfiguration)?
            != b.pin
        {
            return Err(E::InvalidComputationConfiguration);
        }
    }
    Ok(())
}
pub(crate) fn encode(entry: &Entry, resident: &[B]) -> Result<Vec<u8>, E> {
    let mut value = Vec::new();
    value.extend_from_slice(&entry.node.to_le_bytes());
    value.extend_from_slice(&conduit_core::AssignedIdentity::from_text(&entry.key).0);
    if let Some(binding) = resident
        .iter()
        .find(|b| b.node == entry.node && entry.key == "program")
    {
        value.push(4);
        value.extend_from_slice(&binding.pin.text_bytes.to_le_bytes());
        value.extend_from_slice(&binding.pin.text_digest);
        return Ok(value);
    }
    match &entry.value {
        V::Text(text) => {
            value.push(3);
            value.extend_from_slice(text.as_bytes());
        }
        V::Bool(b) => {
            value.push(0);
            value.extend_from_slice(&[u8::from(*b), 0, 0, 0, 0, 0, 0, 0]);
        }
        V::I64(n) => {
            value.push(1);
            value.extend_from_slice(&n.to_le_bytes());
        }
        V::U64(n) => {
            value.push(2);
            value.extend_from_slice(&n.to_le_bytes());
        }
    }
    Ok(value)
}

/// Trusted image inventory for the computation-aware assigned-Plan decoder.
/// Call after fixed-image generation validated each computation's exact contract.
/// Deploy this inventory with the resident image, never derive it from an incoming
/// assignment on the device. These digests do not admit evaluator heap or work.
pub fn assigned_computation_requirements(
    plan: &crate::GeneratedEmbeddedPlan,
) -> Result<Vec<conduit_core::AssignedComputationRequirement>, crate::GenerationError> {
    plan.configuration
        .iter()
        .filter(|entry| matches!(entry.key.as_str(), "program" | "selector"))
        .map(|entry| {
            let crate::GeneratedConfigurationValue::Text(text) = &entry.value else {
                return Err(crate::GenerationError::InvalidComputationConfiguration);
            };
            conduit_core::AssignedComputationRequirement::from_text(entry.node, &entry.key, text)
                .map_err(|_| crate::GenerationError::InvalidComputationConfiguration)
        })
        .collect()
}

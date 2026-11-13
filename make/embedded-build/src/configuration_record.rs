//! One assigned configuration ABI; resident form requires private Make bindings.
use crate::{
    BoundResidentExpression as B, GeneratedConfigurationEntry as Entry,
    GeneratedConfigurationValue as V, GeneratedEmbeddedPlan, GenerationError as E,
};
use conduit_core::AssignedComputationRequirement;
pub(crate) fn validate_bindings(plan: &GeneratedEmbeddedPlan, resident: &[B]) -> Result<(), E> {
    if resident.len() > conduit_core::MAXIMUM_ASSIGNED_COMPUTATIONS {
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
pub(crate) fn encode(entry: &Entry, resident: &[B], pin_selectors: bool) -> Result<Vec<u8>, E> {
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
    // The explicit resident projection already requires an independently linked
    // computation inventory. Selectors use that same exact length/digest ABI;
    // their immutable canonical material stays with the initialized Source Back.
    if pin_selectors && entry.key == "selector" {
        let V::Text(text) = &entry.value else {
            return Err(E::InvalidComputationConfiguration);
        };
        let pin = AssignedComputationRequirement::from_text(entry.node, &entry.key, text)
            .map_err(|_| E::InvalidComputationConfiguration)?;
        value.push(4);
        value.extend_from_slice(&pin.text_bytes.to_le_bytes());
        value.extend_from_slice(&pin.text_digest);
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

/// Encode independently installed software configuration using the same explicit
/// resident ABI as checked assignment generation. This issues no approval or
/// computation inventory; the receiver must retain its own exact Source pins.
pub fn encode_assigned_software_configuration(entry: &Entry) -> Result<Vec<u8>, E> {
    encode(entry, &[], true)
}

/// Trusted image inventory for the computation-aware assigned-Plan decoder.
/// Call after fixed-image generation validated each computation or reference's exact contract.
/// Deploy this inventory with the resident image, never derive it from an incoming
/// assignment on the device. These digests admit neither evaluator heap/work nor
/// access to referenced content.
pub fn assigned_computation_requirements(
    plan: &crate::GeneratedEmbeddedPlan,
) -> Result<Vec<conduit_core::AssignedComputationRequirement>, crate::GenerationError> {
    plan.configuration
        .iter()
        .filter(|entry| matches!(entry.key.as_str(), "program" | "selector" | "reference"))
        .map(|entry| {
            let crate::GeneratedConfigurationValue::Text(text) = &entry.value else {
                return Err(crate::GenerationError::InvalidComputationConfiguration);
            };
            conduit_core::AssignedComputationRequirement::from_text(entry.node, &entry.key, text)
                .map_err(|_| crate::GenerationError::InvalidComputationConfiguration)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_record_ceiling_reports_the_actual_shortage() {
        let fragment = crate::tests::sealed_current_fragment();
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
        let mut plan = crate::generate_embedded_plan(
            &fragment,
            &lowered,
            crate::EmbeddedImageBounds::HOST_TOOLING,
        )
        .unwrap();
        plan.signs = vec![plan.signs[0].clone(); 256];
        assert!(matches!(
            crate::encode_assigned_plan(
                &plan,
                conduit_core::AssignedPlanMaxima {
                    encoded_bytes: u16::MAX,
                    runtime_state_bytes: u16::MAX,
                    counts: [u8::MAX; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
                }
            ),
            Err(E::BoundExceeded {
                table: "assigned Sign records",
                actual: 256,
                maximum: 255
            })
        ));
    }
    #[test]
    fn selector_reference_is_explicit_and_leaves_other_configuration_literal() {
        let entry = Entry {
            node: 7,
            key: "selector".into(),
            value: V::Text("ab".repeat(1024)),
        };
        let literal = encode(&entry, &[], false).unwrap();
        let pinned = encode(&entry, &[], true).unwrap();
        let expected =
            AssignedComputationRequirement::from_text(7, "selector", &"ab".repeat(1024)).unwrap();
        assert_eq!(literal.len() - pinned.len(), 2048 - 36);
        let offset = 2 + core::mem::size_of::<conduit_core::AssignedIdentity>();
        assert_eq!(pinned[offset], 4);
        assert_eq!(
            &pinned[offset + 1..offset + 5],
            &expected.text_bytes.to_le_bytes()
        );
        assert_eq!(&pinned[offset + 5..], &expected.text_digest);
        let other = Entry {
            node: 7,
            key: "label".into(),
            value: V::Text("room".into()),
        };
        assert_eq!(
            encode(&other, &[], false).unwrap(),
            encode(&other, &[], true).unwrap()
        );
        let invalid = Entry {
            node: 7,
            key: "selector".into(),
            value: V::Bool(true),
        };
        assert!(encode(&invalid, &[], true).is_err());
    }
}

//! Selected lossless generic DOM profile; the legacy realization is unchanged.
//! The exclusive pool is cooperative admitted accounting, not an allocator or
//! hostile-code confinement boundary. Configured recipes are not admitted.
use conduit_core::{
    resource_requirement, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostAdvertisement, ImplementationId,
};
use conduit_face::json_model_preflight::{
    face_model_preparation_bytes, generic_face_json_shape, MAX_FACE_JSON_BYTES,
};
use conduit_face::{Face, PlannedMaskPlot};
mod json;
pub use json::encode as bounded_json_bytes;

pub const PROFILE: &str = "browser/owner-face-full@1";
pub const MEMORY_CLASS: &str = "conduit.browser/owner-mask-preparation-bytes@1";
pub const MEMORY_POOL: &str = "browser/owner-mask-preparation";
pub const GENERIC_CAPABILITY: &str = "capability/browser-owner-face-full@1";
/// Explicit Host profile allowance per live/replacement preparation. This is
/// an offered limit, NOT a derivation from the 512 KiB encoded-value ceiling.
/// The input-specific derivation must fit before any model is constructed.
pub const PREPARATION_BYTES: u32 = 16 * 1024 * 1024;
pub const PREPARATION_SLOTS: usize = 2;

pub fn offer() -> CapabilityOffer {
    let mut offer = crate::offer();
    offer.capability_id = CapabilityId::from(GENERIC_CAPABILITY);
    offer.implementation.execution_profile_id = ExecutionProfileId::from(PROFILE);
    offer.implementation.implementation_id =
        ImplementationId::from("implementation/browser-owner-face-full@1");
    offer.implementation.artifact_id = ArtifactId::from("artifact/browser-owner-face-full@1");
    offer
        .resource_requirements
        .push(resource_requirement(MEMORY_CLASS, PREPARATION_BYTES));
    offer
        .resource_requirements
        .sort_by(|a, b| a.class_id.cmp(&b.class_id));
    offer
}

pub fn selected(planned: &PlannedMaskPlot) -> bool {
    planned.show_placement().execution_profile_id.as_str() == PROFILE
}

pub fn planning_host(host: &HostAdvertisement) -> Result<HostAdvertisement, String> {
    let exact = offer();
    if !host.capabilities.contains(&exact) {
        return Err("full_profile Mask offer unavailable".into());
    }
    let selected = HostAdvertisement {
        declared_name: host.declared_name.clone(),
        protocol_version: host.protocol_version,
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        profile: host.profile.clone(),
        bases: vec![],
        planner_capabilities: vec![],
        capabilities: vec![exact],
        resources: host
            .resources
            .iter()
            .filter(|r| {
                r.class_id.as_str() == MEMORY_CLASS
                    || r.class_id.as_str() == conduit_core::FACE_RESOURCE_CLASS
            })
            .cloned()
            .collect(),
    };
    Ok(selected)
}

/// Call under the fixed parser lease. The census and grammar check allocate no
/// model tables; serde's reusable string-escape scratch is charged separately.
pub fn preflight_face(bytes: &[u8]) -> Result<usize, String> {
    let shape =
        generic_face_json_shape(bytes).map_err(|e| format!("full_profile Face census: {e:?}"))?;
    face_model_preparation_bytes(shape).ok_or_else(|| "full_profile Face model extent".into())
}

/// Owner-side eligibility is independent of serialized-length switching.
/// The browser independently rechecks this grammar and input-specific budget.
/// Caller owns these preparation allocations outside its Body Play.
pub fn eligible(face: &Face) -> Result<(), String> {
    if !face.composition.is_empty() {
        return Err("full_profile Mask composition unsupported".into());
    }
    if face.actions.iter().any(|a| !a.arguments.is_empty())
        || face.properties.iter().any(|p| match &p.value {
            conduit_face::FacePropertyValue::TypedValue { contract, .. }
            | conduit_face::FacePropertyValue::ValueContract(contract) => {
                !contract.constraints.is_empty()
            }
            _ => false,
        })
    {
        return Err("full_profile Mask contract family unsupported".into());
    }
    use conduit_core::PrimitiveInfoKind as P;
    for property in &face.properties {
        if let conduit_face::FacePropertyValue::TypedValue { contract, bytes } = &property.value {
            if matches!(
                conduit_core::primitive_info_kind(contract.value_kind.as_str()),
                Some(
                    P::Quantity
                        | P::Distance
                        | P::Frequency
                        | P::Duration
                        | P::Voltage
                        | P::Temperature
                        | P::Angle
                        | P::Ratio
                        | P::PixelCount
                )
            ) && conduit_core::Quantity::decode(bytes).is_err()
            {
                return Err("Full generic wording representation unsupported".into());
            }
        }
    }
    face.validate()
        .map_err(|e| format!("full_profile Face invalid: {e:?}"))?;
    let bytes = bounded_json_bytes(face, MAX_FACE_JSON_BYTES)?;
    let required = preflight_face(&bytes)?;
    if required > PREPARATION_BYTES as usize {
        return Err("full_profile Face preparation pressure".into());
    }
    Ok(())
}

/// Recheck the exact ordinary generic source/placement and exclusive memory
/// binding. The externally selected Lines remain sealed and checked separately.
pub fn validate_planned(host: &HostAdvertisement, planned: &PlannedMaskPlot) -> Result<(), String> {
    if !selected(planned) || planned.mask.plot_name != "browser-graphical" {
        return Err("full_profile Mask profile/source mismatch".into());
    }
    let host = planning_host(host)?;
    let budget = |id: &conduit_core::PortId,
                  direction: conduit_core::PortDirection|
     -> Result<u32, String> {
        let mut ports = planned
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.fore_ports)
            .filter(|p| &p.front_port_id == id && p.direction == direction);
        let p = ports.next().ok_or("full_profile Mask Fore absent")?;
        if ports.next().is_some()
            || p.item_capacity != 1
            || p.byte_capacity > conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_FACE_BYTES
        {
            return Err("full_profile Mask Fore extent".into());
        }
        Ok(p.byte_capacity)
    };
    let expected = crate::planned_mask_with_fore_limits(
        &host,
        crate::MASK_SOURCE,
        "browser-graphical",
        Some(crate::OwnerCarrierForeLimits {
            face_bytes: budget(
                &planned.mask.face_input.front_port_id,
                conduit_core::PortDirection::Input,
            )?,
            show_bytes: budget(
                &planned.mask.show_output.front_port_id,
                conduit_core::PortDirection::Output,
            )?,
        }),
    )?;
    if expected.mask != planned.mask
        || planned.plan.fragments.len() != 1
        || expected.plan.fragments[0].placements != planned.plan.fragments[0].placements
    {
        return Err("full_profile Mask differs from checked generic realization".into());
    }
    let p = planned.show_placement();
    let mut memory = p
        .resources
        .iter()
        .filter(|r| r.class_id.as_str() == MEMORY_CLASS);
    let r = memory
        .next()
        .ok_or("full_profile Mask preparation binding missing")?;
    if memory.next().is_some()
        || r.pool_id.as_str() != MEMORY_POOL
        || r.units != PREPARATION_BYTES
        || r.protected.is_some()
        || r.content.is_some()
        || r.compute.is_some()
        || p.resources
            .iter()
            .any(|other| other.class_id.as_str() != MEMORY_CLASS && other.pool_id == r.pool_id)
    {
        return Err("full_profile Mask preparation binding mismatch".into());
    }
    Ok(())
}

/// Boot preparation only. The immutable template avoids rebuilding catalogs
/// or the planner after a full_profile Face preparation has begun.
pub fn template_plan(host: &HostAdvertisement) -> Result<PlannedMaskPlot, String> {
    crate::planned_mask_with_fore_limits(
        &planning_host(host)?,
        crate::MASK_SOURCE,
        "browser-graphical",
        Some(crate::OwnerCarrierForeLimits {
            face_bytes: conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_FACE_BYTES,
            show_bytes: 60 * 1024,
        }),
    )
}

pub fn validate_template(
    planned: &PlannedMaskPlot,
    template: &PlannedMaskPlot,
) -> Result<(), String> {
    if !selected(planned)
        || planned.mask != template.mask
        || planned.plan.fragments.len() != 1
        || planned.plan.fragments[0].placements != template.plan.fragments[0].placements
    {
        return Err("full_profile Mask differs from prepared generic template".into());
    }
    let f = &planned.plan.fragments[0];
    let t = &template.plan.fragments[0];
    if f.connections != t.connections
        || f.fore_ports.len() != 3
        || f.expected_sign != t.expected_sign
        || f.expected_terminals != t.expected_terminals
        || f.cancellation_policy != t.cancellation_policy
        || f.terminal_policy != t.terminal_policy
        || f.startup_dependencies != t.startup_dependencies
        || f.startup_order != t.startup_order
        || f.sign_storage_budget != t.sign_storage_budget
        || f.source_document_id != t.source_document_id
        || f.checked_plot_id != t.checked_plot_id
        || f.expanded_plot_id != t.expanded_plot_id
        || f.completion_policy != t.completion_policy
    {
        return Err("full_profile Mask generic topology differs".into());
    }
    for p in &f.fore_ports {
        let Some(q) = t.fore_ports.iter().find(|q| {
            q.front_port_id == p.front_port_id && q.direction == p.direction && q.track == p.track
        }) else {
            return Err("full_profile Mask Fore differs".into());
        };
        if p.placement_id != q.placement_id
            || p.gear_port_id != q.gear_port_id
            || p.abnormal_kind != q.abnormal_kind
            || p.pressure_policy != q.pressure_policy
            || p.value_kind != q.value_kind
            || p.value_contract != q.value_contract
            || p.temporal != q.temporal
            || p.item_capacity != q.item_capacity
            || p.byte_capacity != q.byte_capacity
        {
            return Err("full_profile Mask Fore budget differs".into());
        }
    }
    Ok(())
}

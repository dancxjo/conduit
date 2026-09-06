//! Exact sealed Plan, installed offer, and resource admission gates.
use super::*;
use conduit_core::{ResourceAdmissionItem, ResourceAdmissionRequest};

pub(super) fn validate<'a>(
    plan: &'a Plan,
    host: &HostAdvertisement,
    binding: &SessionBinding,
    play: &ActivePlayId,
) -> Result<&'a PlanFragment, String> {
    if !conduit_core::verify_plan(plan)
        || binding.plan_id != plan.plan_id
        || binding.attachment.base.as_str() != "conduit.base/webrtc-data-channel@1"
    {
        return Err("remote execution requires an exact verified WebRTC Plan".into());
    }
    binding.validate().map_err(debug)?;
    let source = plan
        .fragments
        .iter()
        .find(|f| f.fragment_id == binding.source_fragment_id)
        .ok_or("missing source fragment")?;
    let sink = plan
        .fragments
        .iter()
        .find(|f| f.fragment_id == binding.sink_fragment_id)
        .ok_or("missing sink fragment")?;
    let connection = source
        .connections
        .iter()
        .find(|c| c.connection_id == binding.connection_id)
        .ok_or("missing source connection")?;
    if !sink
        .connections
        .iter()
        .any(|candidate| candidate == connection)
    {
        return Err("source and sink do not share the exact planned connection".into());
    }
    let exact = SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
    .map_err(debug)?;
    if &exact != binding {
        return Err("grant differs from the sealed planned connection".into());
    }
    let (fragment, expected_play) =
        if host.host_id == binding.source.host_id && host.boot_id == binding.source.boot_id {
            (source, &binding.source_active_play_id)
        } else if host.host_id == binding.sink.host_id && host.boot_id == binding.sink.boot_id {
            (sink, &binding.sink_active_play_id)
        } else {
            return Err("current Host/Boot does not own this grant".into());
        };
    if expected_play != play
        || fragment.host_id != host.host_id
        || fragment.boot_id != host.boot_id
        || fragment.offer_generation != host.offer_generation
    {
        return Err("stale remote Play or Host generation".into());
    }
    let installed =
        crate::installed_browser::advertisement(host.host_id.clone(), host.boot_id.clone());
    let mut counts = std::collections::BTreeMap::new();
    for gear in &fragment.placements {
        let offer = host
            .capabilities
            .iter()
            .find(|o| o.capability_id == gear.capability_id)
            .ok_or("selected capability absent")?;
        if !installed.capabilities.iter().any(|o| o == offer)
            || gear.host_id != host.host_id
            || gear.boot_id != host.boot_id
            || gear.offer_generation != host.offer_generation
            || gear.implementation_id != offer.implementation.implementation_id
            || gear.artifact_id != offer.implementation.artifact_id
            || gear.execution_profile_id != offer.implementation.execution_profile_id
            || gear.kind_id != offer.kind_id
            || gear.kind_contract_revision != offer.kind_contract_revision
            || gear.limits != offer.limits
            || gear.inputs != offer.inputs
            || gear.outputs != offer.outputs
            || gear.host_operations != offer.host_operations
            || !gear.authority.is_empty()
            || !offer.authority_requirements.is_empty()
        {
            return Err("remote placement differs from the current installed offer".into());
        }
        let count = counts.entry(&gear.capability_id).or_insert(0_u16);
        *count = count.checked_add(1).ok_or("instance count overflow")?;
        if *count > offer.limits.max_active_instances {
            return Err("capability instance limit exceeded".into());
        }
    }
    Ok(fragment)
}

pub(super) fn admit(
    fragment: &PlanFragment,
    host: &HostAdvertisement,
    observations: &[ResourceObservation],
) -> Result<ResourceAdmissionOwner, String> {
    if observations.len() > host.resources.len() {
        return Err("resource observation bound exceeded".into());
    }
    let mut requests = Vec::new();
    for gear in &fragment.placements {
        let offer = host
            .capabilities
            .iter()
            .find(|o| o.capability_id == gear.capability_id)
            .ok_or("capability absent")?;
        if gear.resources.len() != offer.resource_requirements.len() {
            return Err("resource requirement count differs".into());
        }
        let mut items = Vec::new();
        for requirement in &offer.resource_requirements {
            let matches = gear
                .resources
                .iter()
                .filter(|r| r.class_id == requirement.class_id)
                .collect::<Vec<_>>();
            let [binding] = matches.as_slice() else {
                return Err("missing or duplicate resource binding".into());
            };
            if binding.protected.is_some() {
                return Err(
                    "protected resources require a separate admitted authority entrance".into(),
                );
            }
            items.push(ResourceAdmissionItem {
                requirement: requirement.clone(),
                binding: (*binding).clone(),
            });
        }
        if !items.is_empty() {
            requests.push(ResourceAdmissionRequest {
                plan_id: fragment.plan_id.clone(),
                placement_id: gear.placement_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                items,
            });
        }
    }
    let mut owner = ResourceAdmissionOwner::new(host.clone());
    if !requests.is_empty() {
        owner.admit_batch(requests, observations).map_err(debug)?;
    }
    Ok(owner)
}

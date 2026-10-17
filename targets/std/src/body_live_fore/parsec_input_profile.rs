//! Exact combined Parsec preparation profile, not input binding authority.
use conduit_core::{
    verify_plan, CapabilityOffer, ConnectionTrack, FrontValueLocation, HostAdvertisement, Plan,
    PlannedActivationEntry, PlannedForePort, PlannedGear, PortDirection, PortTemporal,
};
use conduit_face::PARSEC_MAP_FRAME_BYTES;

pub(super) fn validate(plan: &Plan, host: &HostAdvertisement) -> Result<(), String> {
    if !verify_plan(plan) || plan.fragments.len() != 1 || plan.activations.len() != 1 {
        return Err("Parsec input profile requires one sealed local scan Plan".into());
    }
    let fragment = &plan.fragments[0];
    let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
        return Err("Parsec input profile requires the exact scan activation".into());
    };
    if fragment.host_id != host.host_id
        || fragment.boot_id != host.boot_id
        || fragment.offer_generation != host.offer_generation
        || fragment.placements.len() != 3
        || fragment.fore_ports.len() != 4
        || !fragment.connections.is_empty()
        || !fragment.states.is_empty()
    {
        return Err(
            "Parsec input profile differs from its current local three-placement shape".into(),
        );
    }
    let owner = fragment
        .placements
        .iter()
        .find(|p| p.placement_id == scan.owner_placement_id)
        .ok_or("Parsec input profile has no scan owner")?;
    crate::flow_activation::validate_planned_parsec_scan(owner, scan)?;
    crate::flow_activation::maximum_scan_child_steps(scan)?;
    let keyboard = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND)
        .ok_or("Parsec input profile has no raw keyboard Source")?;
    let button = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::BUTTON_SOURCE_KIND)
        .ok_or("Parsec input profile has no Space button Source")?;
    if keyboard.placement_id == button.placement_id
        || keyboard.placement_id == owner.placement_id
        || button.placement_id == owner.placement_id
    {
        return Err("Parsec input profile duplicates a source or scan placement".into());
    }
    for placement in [owner, keyboard, button] {
        let offer = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == placement.capability_id)
            .ok_or("Parsec input profile selected offer is absent")?;
        match_offer(placement, offer, host)?;
    }
    let keyboard_offer = host
        .capabilities
        .iter()
        .find(|offer| offer.capability_id == keyboard.capability_id)
        .ok_or("Parsec keyboard offer is absent")?;
    if keyboard_offer
        != &conduit_std_offers::hosted_keyboard_offer(
            keyboard.capability_id.as_str(),
            keyboard.artifact_id.as_str(),
        )
        || host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == button.capability_id)
            != Some(&conduit_std_offers::button::offer())
    {
        return Err("Parsec input profile selects a different input realization".into());
    }
    let command = crate::flow_activation::parsec_command_kind()?;
    let result = crate::flow_activation::parsec_result_kind()?;
    check_fore(
        fragment
            .fore_ports
            .iter()
            .find(|p| p.front_port_id.as_str() == "commands"),
        owner,
        "item",
        PortDirection::Input,
        PortTemporal::Flow { closes: true },
        &command,
        PARSEC_MAP_FRAME_BYTES as u32,
    )?;
    check_fore(
        fragment
            .fore_ports
            .iter()
            .find(|p| p.front_port_id.as_str() == "states"),
        owner,
        "combined",
        PortDirection::Output,
        PortTemporal::Flow { closes: true },
        &result,
        PARSEC_MAP_FRAME_BYTES as u32,
    )?;
    check_fore(
        fragment
            .fore_ports
            .iter()
            .find(|p| p.front_port_id.as_str() == "keys"),
        keyboard,
        conduit_semantic_catalog::KEYBOARD_PORT,
        PortDirection::Output,
        PortTemporal::Flow { closes: false },
        &keyboard.outputs[0].value_kind,
        conduit_human::KEY_EVENT_ENCODED_LEN as u32,
    )?;
    check_fore(
        fragment
            .fore_ports
            .iter()
            .find(|p| p.front_port_id.as_str() == "fire"),
        button,
        button.outputs[0].port_id.as_str(),
        PortDirection::Output,
        PortTemporal::Flow { closes: false },
        &button.outputs[0].value_kind,
        conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
    )?;
    Ok(())
}

fn match_offer(
    p: &PlannedGear,
    offer: &CapabilityOffer,
    host: &HostAdvertisement,
) -> Result<(), String> {
    if p.host_id != host.host_id
        || p.boot_id != host.boot_id
        || p.offer_generation != host.offer_generation
        || p.kind_id != offer.kind_id
        || p.kind_contract_revision != offer.kind_contract_revision
        || p.execution_profile_id != offer.implementation.execution_profile_id
        || p.implementation_id != offer.implementation.implementation_id
        || p.artifact_id != offer.implementation.artifact_id
        || p.inputs != offer.inputs
        || p.outputs != offer.outputs
        || p.host_calls != offer.host_calls
        || p.semantic_contract != offer.semantic_contract
        || p.limits != offer.limits
        || !p.configuration.is_empty()
        || p.base.is_some()
        || !p.authority.is_empty()
        || !p.realization_characteristics.is_empty()
        || !p.realization_properties.is_empty()
        || !p.terminal_transductions.is_empty()
        || !p.pool_references.is_empty()
        || p.resources.len() != offer.resource_requirements.len()
        || p.resources
            .iter()
            .zip(&offer.resource_requirements)
            .any(|(binding, requirement)| {
                binding.class_id != requirement.class_id
                    || binding.units != requirement.units
                    || binding.protected.is_some()
                    || binding.compute.is_some()
            })
    {
        return Err("Parsec input placement differs from the actual current offer".into());
    }
    Ok(())
}

fn check_fore(
    port: Option<&PlannedForePort>,
    owner: &PlannedGear,
    gear_port: &str,
    direction: PortDirection,
    temporal: PortTemporal,
    kind: &conduit_core::KindId,
    bytes: u32,
) -> Result<(), String> {
    let port = port.ok_or("Parsec input profile is missing an exact Fore")?;
    // Match the actual selected Front envelope independently of staging capacity.
    // Keyboard/button offers have no explicit envelope; scan Fores retain theirs.
    let location = match direction {
        PortDirection::Input => FrontValueLocation::Input(conduit_core::port_id(gear_port)),
        PortDirection::Output => FrontValueLocation::Output(conduit_core::port_id(gear_port)),
    };
    let contract = owner
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|contract| contract.location == location)
        .map(|contract| &contract.contract);
    if port.placement_id != owner.placement_id
        || port.gear_port_id.as_str() != gear_port
        || port.direction != direction
        || port.temporal != temporal
        || port.value_kind != *kind
        || port.track != ConnectionTrack::Payload
        || port.selected_line.is_some()
        || port.abnormal_kind.is_some()
        || port.item_capacity != 1
        || port.byte_capacity != bytes
        || port.value_contract.as_ref() != contract
    {
        return Err("Parsec input profile Fore differs from its exact bounded endpoint".into());
    }
    Ok(())
}

//! Exact finite scan initialization for flat, pure, local Value child plans.
use super::{install_planned_activation, LocalExpressionFactory, StdActivationHost};
use conduit_composite::{BoundedScanActivationHost, KernelOperationFactory};
use conduit_core::*;

pub const VALUE_SCAN_IMPLEMENTATION: &str = "std/kernel-value-scan@1";
pub const VALUE_SCAN_PROFILE: &str = "std/value-scan-prepared@1";
pub const VALUE_SCAN_ARTIFACT: &str = "conduit-composite/kernel-scan@1";
pub const VALUE_SCAN_MAXIMUM_ITEMS: u16 = 64;

pub fn value_scan_offer(
    item: &CheckedValueContract,
    accumulator: &CheckedValueContract,
    initial: &[u8],
    maximum_items: u16,
) -> Result<CapabilityOffer, String> {
    if maximum_items == 0 || maximum_items > VALUE_SCAN_MAXIMUM_ITEMS {
        return Err("Value scan admits 1..=64 items".into());
    }
    let kind = conduit_semantic_catalog::flow_scan_semantic_contract(
        item,
        accumulator,
        initial,
        None,
        maximum_items,
    )
    .map_err(str::to_string)?;
    let initial_digest = semantic_digest("std/value-scan-initial@1", initial);
    let front = compute_checked_front_fingerprint(&kind.checked_front());
    let mut identity_bytes = [0_u8; 98];
    identity_bytes[..32].copy_from_slice(&initial_digest);
    identity_bytes[32..96].copy_from_slice(front.as_bytes());
    identity_bytes[96..].copy_from_slice(&maximum_items.to_le_bytes());
    let identity = semantic_digest("std/value-scan-profile@1", &identity_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: format!("std/value-scan/{identity}/{maximum_items}").into(),
            execution_profile_id: VALUE_SCAN_PROFILE.into(),
            implementation_id: VALUE_SCAN_IMPLEMENTATION.into(),
            artifact_id: VALUE_SCAN_ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}

/// The registry retains exact Source schema specializations before this call.
/// Receipt transfer and execution use the existing activation/kernel machinery.
pub fn install_value_scan(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut StdActivationHost,
) -> Result<BoundedScanActivationHost, String> {
    if !verify_plan(plan) {
        return Err("Value scan requires a verified whole Plan".into());
    }
    let selected = plan
        .activations
        .iter()
        .find(|entry| match entry {
            PlannedActivationEntry::Unary(value) => value.activation_id == activation_id,
            PlannedActivationEntry::Fold(value) => value.activation_id == activation_id,
            PlannedActivationEntry::Scan(value) => value.activation_id == activation_id,
        })
        .ok_or("Value scan activation is absent")?;
    let PlannedActivationEntry::Scan(scan) = selected else {
        return Err("Value scan requires a Scan activation".into());
    };
    let owner = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|gear| gear.placement_id == scan.owner_placement_id)
        .ok_or("Value scan owner is absent")?;
    let contracts = owner.semantic_contract.value_contracts();
    let item = contracts
        .iter()
        .find(|v| v.location == FrontValueLocation::Input("item".into()))
        .ok_or("Value scan item contract is absent")?;
    let accumulator = contracts
        .iter()
        .find(|v| v.location == FrontValueLocation::Output("combined".into()))
        .ok_or("Value scan accumulator contract is absent")?;
    let expected = value_scan_offer(
        &item.contract,
        &accumulator.contract,
        &scan.initial_accumulator,
        scan.limits.maximum_items,
    )?;
    if owner.kind_id != expected.kind_id
        || owner.kind_contract_revision != expected.kind_contract_revision
        || owner.capability_id != expected.capability_id
        || owner.implementation_id != expected.implementation.implementation_id
        || owner.execution_profile_id != expected.implementation.execution_profile_id
        || owner.artifact_id != expected.implementation.artifact_id
        || owner.inputs != expected.inputs
        || owner.outputs != expected.outputs
        || owner.semantic_contract != expected.semantic_contract
        || owner.limits != expected.limits
        || !owner.configuration.is_empty()
        || owner.base.is_some()
        || !owner.host_calls.is_empty()
        || !owner.resources.is_empty()
        || !owner.authority.is_empty()
        || !owner.pool_references.is_empty()
        || !owner.realization_properties.is_empty()
        || !owner.realization_characteristics.is_empty()
        || !owner.terminal_transductions.is_empty()
    {
        return Err("Value scan owner differs from its exact prepared profile".into());
    }
    let child = &scan.selected_plan;
    if child.fragments.first().is_none_or(|fragment| {
        fragment.host_id != owner.host_id
            || fragment.boot_id != owner.boot_id
            || fragment.offer_generation != owner.offer_generation
    }) {
        return Err("Value scan child belongs to another Host generation".into());
    }
    value_scan_child_steps(scan)?;
    install_planned_activation(plan, prepared, activation_id, host)
        .map_err(|error| format!("Value scan child receipt transfer: {error:?}"))?
        .into_scan()
        .map_err(|error| format!("Value scan preparation: {error:?}"))
}

/// Work admission only: this validates the already selected graph, never
/// constructs connectivity, chooses a Back, schedules a node, or evaluates DSP.
pub fn value_scan_child_steps(scan: &PlannedScanActivation) -> Result<u32, String> {
    if scan.limits.maximum_items == 0 || scan.limits.maximum_items > VALUE_SCAN_MAXIMUM_ITEMS {
        return Err("Value scan item bound is outside this profile".into());
    }
    let child = &scan.selected_plan;
    if !verify_plan(child)
        || scan.selected_plan_id != child.plan_id
        || !child.activations.is_empty()
        || child.fragments.len() != 1
    {
        return Err("Value scan requires one exact non-nested local child Plan".into());
    }
    let fragment = &child.fragments[0];
    if fragment.placements.is_empty()
        || fragment.placements.len() > 64
        || fragment.connections.len() > 128
        || !fragment.states.is_empty()
        || !fragment.shared_pools.is_empty()
        || !fragment.execution_regions.is_empty()
        || !fragment.execution_fusions.is_empty()
        || fragment.fore_ports.len() != 3
        || fragment.terminal_policy != TerminalPolicy::RequireAllPlacementsAndConnections
        || fragment.cancellation_policy != CancellationPolicy::CancelAllAndRejectLateCompletion
    {
        return Err("Value scan child exceeds its flat finite profile".into());
    }
    for (expected, direction, bytes) in [
        (
            &scan.accumulator_input,
            PortDirection::Input,
            scan.retained_accumulator_bytes,
        ),
        (
            &scan.item_input,
            PortDirection::Input,
            scan.retained_item_bytes,
        ),
        (
            &scan.output,
            PortDirection::Output,
            scan.retained_accumulator_bytes,
        ),
    ] {
        if !fragment.fore_ports.iter().any(|port| {
            port.front_port_id == expected.front_port_id
                && port.direction == direction
                && port.value_kind == expected.value_kind
                && port.abnormal_kind.is_none()
                && port.temporal == PortTemporal::Value
                && port.track == ConnectionTrack::Payload
                && port.selected_line.is_none()
                && port.item_capacity == 1
                && port.byte_capacity >= bytes
        }) {
            return Err("Value scan child requires exact local Value Fores".into());
        }
    }
    for gear in &fragment.placements {
        if !gear.host_calls.is_empty()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
            || gear.base.is_some()
            || !gear.pool_references.is_empty()
            || gear
                .inputs
                .iter()
                .chain(&gear.outputs)
                .any(|port| port.temporal != PortTemporal::Value || port.abnormal_kind.is_some())
        {
            return Err(
                "Value scan child may not suspend on external effects or temporal state".into(),
            );
        }
        match gear.implementation_id.as_str() {
            conduit_std_offers::PURE_EXPRESSION_LOCAL_IMPLEMENTATION => {
                LocalExpressionFactory.budget(gear)?;
            }
            conduit_std_offers::VALUE_PAIR_IMPLEMENTATION
                if gear.artifact_id.as_str() == conduit_std_offers::VALUE_PAIR_ARTIFACT
                    && gear.execution_profile_id.as_str()
                        == conduit_std_offers::VALUE_PAIR_PROFILE => {}
            _ => {
                return Err(
                    "Value scan child implementation has no admitted finite step profile".into(),
                )
            }
        }
    }
    let index = |id: &PlacementId| {
        fragment
            .placements
            .iter()
            .position(|gear| &gear.placement_id == id)
            .ok_or("Value scan Cord has no local endpoint")
    };
    let mut incoming = [0_u16; 64];
    for cord in &fragment.connections {
        if cord.temporal != PortTemporal::Value
            || cord.track != ConnectionTrack::Payload
            || cord.abnormal_kind.is_some()
            || cord.selected_line.is_some()
            || !cord.admitted_lines.is_empty()
            || cord.resource.is_some()
        {
            return Err("Value scan Cords must be local payload-only Values".into());
        }
        index(&cord.source_placement_id)?;
        incoming[index(&cord.sink_placement_id)?] += 1;
    }
    let mut removed = [false; 64];
    for _ in &fragment.placements {
        let next = (0..fragment.placements.len())
            .find(|i| !removed[*i] && incoming[*i] == 0)
            .ok_or("Value scan child must be acyclic")?;
        removed[next] = true;
        for cord in &fragment.connections {
            if cord.source_placement_id == fragment.placements[next].placement_id {
                incoming[index(&cord.sink_placement_id)?] -= 1;
            }
        }
    }
    // Each supported Back emits once and completes once. The conservative
    // remaining allowance covers local Cord/Fore terminal handoff per item.
    let per_item = 4_u32
        .checked_mul(fragment.placements.len() as u32)
        .and_then(|steps| steps.checked_add(2 * fragment.connections.len() as u32 + 8))
        .ok_or("Value scan step bound overflow")?;
    per_item
        .checked_mul(u32::from(scan.limits.maximum_items))
        .ok_or_else(|| "Value scan total step bound overflow".into())
}

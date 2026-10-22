//! Preparation-only binding of the canonical Source Fore to its sole owner.
//! This offers no second actor and does not install a scheduler or change Source.
use super::WorksetRefusal;
use alloc::{format, vec};
use conduit_core::{
    Back, BackOfferBuilder, BootId, CapabilityOffer, ConnectionTrack, HostCallRequirement, HostId,
    OfferGeneration, Plan, PlanId, PortDirection, PortTemporal, resource_requirement,
};
use conduit_kernel::NodeId;
use conduit_plan_lowering::lowering::{
    LoweredForePort, LoweredPlanFragment, lower_plan_fragment_from_plan,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

pub(super) const IMPLEMENTATION: &str = "conduitos/patchbay-accept-interaction@1";
pub(super) const OPERATION: &str = "conduit.host/conduitos-patchbay-accept-interaction@1";
const BYTES: u32 = conduit_face::MAX_FACE_INTERACTION_BYTES as u32;
const RESOURCE: &str = "conduit.resource/runtime-memory@1";
// Reserve the exact maximum input payload. Sink bookkeeping belongs to the
// existing fixed Runtime tables; actor storage stays in its admitted service.
const MEMORY: u32 = BYTES;

pub(super) fn offer(build: &str) -> CapabilityOffer {
    BackOfferBuilder::new(
        patchbay_application::patchbay_interaction_contract(),
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: "conduitos/bounded-patchbay-interaction@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: format!("conduitos-build/{build}").into(),
            host_calls: vec![HostCallRequirement {
                contract_id: OPERATION.into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: BYTES,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![resource_requirement(RESOURCE, MEMORY)],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub(super) fn install_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), WorksetRefusal> {
    conduit_face::install_mask_plot_value_aliases(startup).map_err(|_| WorksetRefusal::Catalog)?;
    let kind = patchbay_application::patchbay_interaction_contract();
    startup
        .insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|_| WorksetRefusal::Catalog)?;
    startup
        .insert_fore(
            patchbay_application::PATCHBAY_INTERACTION_KIND,
            kind.checked_front(),
        )
        .map_err(|_| WorksetRefusal::Catalog)?;
    profile
        .insert_kind(kind)
        .map_err(|_| WorksetRefusal::Catalog)
}

pub(super) struct Ingress {
    pub input: LoweredForePort,
    pub node: NodeId,
    pub plan_id: PlanId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

pub(super) fn admit(
    plan: &Plan,
    lowered: &LoweredPlanFragment,
) -> Result<Option<Ingress>, WorksetRefusal> {
    if !plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|gear| gear.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND)
    {
        return Ok(None);
    }
    let [fragment] = plan.fragments.as_slice() else {
        return Err(WorksetRefusal::Plan);
    };
    let [gear] = fragment.placements.as_slice() else {
        return Err(WorksetRefusal::Plan);
    };
    let expected = offer("");
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.semantic_contract != expected.semantic_contract
        || gear.host_calls != expected.host_calls
        || gear.limits != expected.limits
        || gear.host_id != fragment.host_id
        || gear.boot_id != fragment.boot_id
        || gear.offer_generation != fragment.offer_generation
        || !gear.configuration.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.terminal_transductions.is_empty()
        || !gear.pool_references.is_empty()
        || !fragment.connections.is_empty()
        || !fragment.states.is_empty()
        || !fragment.execution_fusions.is_empty()
    {
        return Err(WorksetRefusal::Plan);
    }
    let [memory] = gear.resources.as_slice() else {
        return Err(WorksetRefusal::Resource);
    };
    if memory.class_id.as_str() != RESOURCE
        || memory.units != MEMORY
        || memory.protected.is_some()
        || memory.compute.is_some()
        || memory.content.is_some()
    {
        return Err(WorksetRefusal::Resource);
    }
    let [fore] = fragment.fore_ports.as_slice() else {
        return Err(WorksetRefusal::Plan);
    };
    if fore.front_port_id.as_str() != "interaction"
        || fore.placement_id != gear.placement_id
        || fore.gear_port_id.as_str() != "interaction"
        || fore.direction != PortDirection::Input
        || fore.track != ConnectionTrack::Payload
        || fore.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
        || fore.temporal != (PortTemporal::Flow { closes: true })
        || fore.abnormal_kind.is_some()
        || fore.selected_line.is_some()
        || fore.item_capacity != 1
        || fore.byte_capacity != BYTES
    {
        return Err(WorksetRefusal::Plan);
    }
    // Fragment-set lowering reindexes numeric nodes, Cords and endpoints.
    // Normalize the independently lowered shape to the actual admitted indices;
    // retain those actual indices rather than inventing a detached kernel.
    let (mut exact, activations) = lower_plan_fragment_from_plan(plan, &fragment.fragment_id)
        .map_err(|_| WorksetRefusal::Lowering)?;
    if !activations.entries.is_empty() {
        return Err(WorksetRefusal::Plan);
    }
    let [input] = lowered.fore_ports.as_slice() else {
        return Err(WorksetRefusal::Lowering);
    };
    let [node] = lowered.nodes.as_slice() else {
        return Err(WorksetRefusal::Lowering);
    };
    let [cord] = lowered.cords.as_slice() else {
        return Err(WorksetRefusal::Lowering);
    };
    let [call] = lowered.host_calls.as_slice() else {
        return Err(WorksetRefusal::Lowering);
    };
    if lowered.identity.plan_id != fragment.plan_id
        || lowered.identity.fragment_id != fragment.fragment_id
        || lowered.identity.placements.as_slice() != [(node.node, gear.placement_id.clone())]
        || !lowered.remote_endpoints.is_empty()
        || lowered.node_specs.len() != 1
    {
        return Err(WorksetRefusal::Lowering);
    }
    exact.nodes[0].node = node.node;
    exact.nodes[0].inputs[0].node = node.node;
    exact.fore_ports[0].endpoint = input.endpoint;
    exact.fore_ports[0].cord = input.cord;
    exact.cords[0].spec.cord = input.cord;
    exact.cords[0].spec.source = conduit_kernel::CordEndpoint::Remote(input.endpoint);
    exact.cords[0].spec.sink = conduit_kernel::CordEndpoint::Local {
        node: node.node,
        port: exact.nodes[0].inputs[0].port,
    };
    // Aggregate slot ranges are owned and checked by fragment-set admission.
    exact.cords[0].spec.slot_start = cord.spec.slot_start;
    exact.node_specs[0].input_cords[0] = Some(input.cord);
    exact.host_calls[0].node = node.node;
    exact.host_calls[0].call = call.call;
    exact.host_calls[0].binding.call = call.call;
    if lowered.completion_policy != exact.completion_policy
        || lowered.cord_value_slots != exact.cord_value_slots
        || lowered.cord_value_bytes != exact.cord_value_bytes
        || lowered.sign_items != exact.sign_items
        || lowered.sign_bytes != exact.sign_bytes
        || !lowered.routes.is_empty()
        || !lowered.states.is_empty()
        || !lowered.fusions.is_empty()
        || !lowered.shared_pools.is_empty()
        || lowered.fore_ports != exact.fore_ports
        || lowered.nodes != exact.nodes
        || lowered.node_specs != exact.node_specs
        || lowered.host_calls != exact.host_calls
        || lowered.cords != exact.cords
    {
        return Err(WorksetRefusal::Lowering);
    }
    Ok(Some(Ingress {
        input: input.clone(),
        node: node.node,
        plan_id: fragment.plan_id.clone(),
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
    }))
}

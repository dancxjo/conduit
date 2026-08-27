//! Exact ordinary Form and Plan preparation for bounded scalar latest/tee.

use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection,
    PortTemporal, Scalar, kind_id, port_id,
};
use conduit_form::{ProfileCatalog, StartupCatalog, parse};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use super::flow_state_play::FlowStateError;

pub(super) const SOURCE_KIND: &str = "conduitos/fixture-flow-source";
pub(super) const LEFT_SINK_KIND: &str = "conduitos/fixture-flow-left-sink";
pub(super) const RIGHT_SINK_KIND: &str = "conduitos/fixture-flow-right-sink";
const SOURCE_REVISION: &str = "conduitos/fixture-flow-source@1";
const SINK_REVISION: &str = "conduitos/fixture-flow-sink@1";
const FIXTURE_ARTIFACT: &str = "conduitos/flow-state-fixture@1";
pub(super) const SINK_HOST_OPERATION: &str = "conduitos.fixture/capture-flow-scalar@1";

pub struct PreparedFlowState {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub(super) value: Scalar,
}

pub fn prepare_flow_state(
    host: &str,
    boot: &str,
    value: Scalar,
) -> Result<PreparedFlowState, FlowStateError> {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_flow_state_catalogs(&mut startup, &mut catalog)
        .map_err(|_| FlowStateError::Catalog)?;
    for offer in [
        source_offer(value),
        sink_offer(LEFT_SINK_KIND),
        sink_offer(RIGHT_SINK_KIND),
    ] {
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .map_err(|_| FlowStateError::Catalog)?;
    }
    let source = format!(
        "form flow_state {{\n source: {SOURCE_KIND}\n latest: state/latest\n tee: flow/tee\n left: {LEFT_SINK_KIND}\n right: {RIGHT_SINK_KIND}\n source.value > latest.in\n latest.out > tee.in\n tee.left > left.value\n tee.right > right.value\n}}\n"
    );
    let form = parse(&source, &catalog).map_err(|_| FlowStateError::Form)?;
    let advertisement = advertisement(host, boot, value);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| FlowStateError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| FlowStateError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(FlowStateError::Plan);
    }
    Ok(PreparedFlowState {
        advertisement,
        plan,
        value,
    })
}

fn advertisement(host: &str, boot: &str, value: Scalar) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities: vec![
            source_offer(value),
            crate::functional_offers::state_latest_scalar_offer(),
            crate::functional_offers::flow_tee_scalar_offer(),
            sink_offer(LEFT_SINK_KIND),
            sink_offer(RIGHT_SINK_KIND),
        ],
    }
}

fn source_offer(value: Scalar) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!(
            "conduitos-fixture-flow-source-{}@1",
            value.raw_microunits()
        )),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: fixture_implementation("conduitos.fixture/flow-source@1"),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: limits(),
    }
}

fn sink_offer(kind: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("{}-capability@1", kind.replace('/', "-"))),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(SINK_REVISION),
        implementation: fixture_implementation("conduitos.fixture/flow-sink@1"),
        inputs: vec![scalar_port("value", PortDirection::Input)],
        outputs: Vec::new(),
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(SINK_HOST_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: limits(),
    }
}

fn fixture_implementation(id: &str) -> conduit_core::ImplementationOffer {
    conduit_core::ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from(
            crate::functional_offers::FLOW_STATE_PROFILE,
        ),
        implementation_id: ImplementationId::from(id),
        artifact_id: ArtifactId::from(FIXTURE_ARTIFACT),
    }
}

fn scalar_port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
        direction,
        temporal: PortTemporal::Current,
    }
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
    }
}

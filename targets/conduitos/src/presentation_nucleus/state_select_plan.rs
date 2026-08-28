//! Ordinary Form and immutable Plan preparation for portable `state/select`.

use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId, InfoBool,
    KindContractRevision, OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection,
    PortTemporal, Scalar, kind_id, port_id,
};
use conduit_form::{ProfileCatalog, StartupCatalog, parse};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use super::state_select_play::StateSelectError;

pub(super) const SELECTOR_SOURCE_KIND: &str = "conduitos/fixture-select-bool-source";
pub(super) const FALSE_SOURCE_KIND: &str = "conduitos/fixture-select-false-source";
pub(super) const TRUE_SOURCE_KIND: &str = "conduitos/fixture-select-true-source";
pub(super) const SINK_KIND: &str = "conduitos/fixture-select-sink";
pub(super) const SINK_HOST_OPERATION: &str = "conduitos.fixture/capture-selected-scalar@1";
const SOURCE_ADVANCE_HOST_OPERATION: &str = "conduitos.fixture/advance-select-source@1";
const SOURCE_REVISION: &str = "conduitos/fixture-select-source@1";
const SINK_REVISION: &str = "conduitos/fixture-select-sink@1";
const FIXTURE_ARTIFACT: &str = "conduitos/state-select-fixture@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateSelectSequence {
    pub selectors: [Option<InfoBool>; 2],
    pub when_false: [Option<Scalar>; 2],
    pub when_true: [Option<Scalar>; 2],
}

impl StateSelectSequence {
    pub const fn one(selector: InfoBool, when_false: Scalar, when_true: Scalar) -> Self {
        Self {
            selectors: [Some(selector), None],
            when_false: [Some(when_false), None],
            when_true: [Some(when_true), None],
        }
    }
}

pub struct PreparedStateSelect {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub(super) sequence: StateSelectSequence,
}

pub fn prepare_state_select(
    host: &str,
    boot: &str,
    sequence: StateSelectSequence,
) -> Result<PreparedStateSelect, StateSelectError> {
    validate_sequence(&sequence)?;
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut catalog)
        .map_err(|_| StateSelectError::Catalog)?;
    for offer in source_and_sink_offers(sequence) {
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .map_err(|_| StateSelectError::Catalog)?;
    }
    let source = format!(
        "form state_select {{\n selector: {SELECTOR_SOURCE_KIND}\n when_false: {FALSE_SOURCE_KIND}\n when_true: {TRUE_SOURCE_KIND}\n select: state/select\n sink: {SINK_KIND}\n selector.value > select.selector\n when_false.value > select.when-false\n when_true.value > select.when-true\n select.out > sink.value\n}}\n"
    );
    let form = parse(&source, &catalog).map_err(|_| StateSelectError::Form)?;
    let advertisement = advertisement(host, boot, sequence);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| StateSelectError::Placement)?;
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
    .map_err(|_| StateSelectError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(StateSelectError::Plan);
    }
    Ok(PreparedStateSelect {
        advertisement,
        plan,
        sequence,
    })
}

fn validate_sequence(sequence: &StateSelectSequence) -> Result<(), StateSelectError> {
    for values_present in [
        sequence.selectors.map(|value| value.is_some()),
        sequence.when_false.map(|value| value.is_some()),
        sequence.when_true.map(|value| value.is_some()),
    ] {
        if !values_present[0] {
            return Err(StateSelectError::Value);
        }
    }
    Ok(())
}

fn advertisement(host: &str, boot: &str, sequence: StateSelectSequence) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities: vec![
            bool_source_offer(sequence.selectors),
            scalar_source_offer(FALSE_SOURCE_KIND, sequence.when_false),
            scalar_source_offer(TRUE_SOURCE_KIND, sequence.when_true),
            crate::functional_offers::state_select_scalar_offer(),
            sink_offer(),
        ],
    }
}

fn source_and_sink_offers(sequence: StateSelectSequence) -> [CapabilityOffer; 4] {
    [
        bool_source_offer(sequence.selectors),
        scalar_source_offer(FALSE_SOURCE_KIND, sequence.when_false),
        scalar_source_offer(TRUE_SOURCE_KIND, sequence.when_true),
        sink_offer(),
    ]
}

fn bool_source_offer(values: [Option<InfoBool>; 2]) -> CapabilityOffer {
    source_offer(
        SELECTOR_SOURCE_KIND,
        conduit_core::BOOL_INFO_ID,
        values
            .map(|value| value.map_or(2, |value| u8::from(value.get())))
            .map(|value| value.to_string())
            .join("-"),
    )
}

fn scalar_source_offer(kind: &str, values: [Option<Scalar>; 2]) -> CapabilityOffer {
    source_offer(
        kind,
        conduit_core::SCALAR_INFO_ID,
        values
            .map(|value| {
                value.map_or_else(|| "none".into(), |value| value.raw_microunits().to_string())
            })
            .join("-"),
    )
}

fn source_offer(kind: &str, value_kind: &str, identity: String) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("{}-{identity}@1", kind.replace('/', "-"))),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: fixture_implementation("conduitos.fixture/state-select-source@1"),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(value_kind),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(SOURCE_ADVANCE_HOST_OPERATION),
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

fn sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-state-select-sink@1"),
        kind_id: kind_id(SINK_KIND),
        kind_contract_revision: KindContractRevision::from(SINK_REVISION),
        implementation: fixture_implementation("conduitos.fixture/state-select-sink@1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(SINK_HOST_OPERATION),
            target_kind: Some(kind_id(SINK_KIND)),
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

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
    }
}

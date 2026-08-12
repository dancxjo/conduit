//! Exact ordinary Form and Plan preparation for bounded multi-input logic.

use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection,
    PortTemporal, Scalar, kind_id, port_id,
};
use conduit_form::{ProfileCatalog, StartupCatalog, parse};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use super::logic_multi_play::LogicMultiError;

pub(super) const LEFT_KIND: &str = "conduitos/fixture-logic-left";
pub(super) const RIGHT_KIND: &str = "conduitos/fixture-logic-right";
pub(super) const FALSE_KIND: &str = "conduitos/fixture-logic-false";
pub(super) const TRUE_KIND: &str = "conduitos/fixture-logic-true";
pub(super) const SINK_KIND: &str = "conduitos/fixture-logic-sink";
const SOURCE_REVISION: &str = "conduitos/fixture-logic-scalar-source@1";
const SINK_REVISION: &str = "conduitos/fixture-logic-scalar-sink@1";
const FIXTURE_ARTIFACT: &str = "conduitos/logic-multi-fixture@1";
pub(super) const SINK_HOST_OPERATION: &str = "conduitos.fixture/capture-scalar@1";

pub struct PreparedLogicMulti {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub(super) left: Scalar,
    pub(super) right: Scalar,
    pub(super) when_false: Scalar,
    pub(super) when_true: Scalar,
    pub(super) comparison: conduit_std_catalog::ScalarComparison,
}

pub fn prepare_logic_multi(
    host: &str,
    boot: &str,
    left: Scalar,
    right: Scalar,
    comparison: conduit_std_catalog::ScalarComparison,
    when_false: Scalar,
    when_true: Scalar,
) -> Result<PreparedLogicMulti, LogicMultiError> {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_logic_catalogs(&mut startup, &mut catalog)
        .map_err(|_| LogicMultiError::Catalog)?;
    let sources = [
        (LEFT_KIND, left),
        (RIGHT_KIND, right),
        (FALSE_KIND, when_false),
        (TRUE_KIND, when_true),
    ];
    for (kind, value) in sources {
        let offer = source_offer(kind, value);
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
                inputs: Vec::new(),
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .map_err(|_| LogicMultiError::Catalog)?;
    }
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .map_err(|_| LogicMultiError::Catalog)?;
    let operator = match comparison {
        conduit_std_catalog::ScalarComparison::Less => "lt",
        conduit_std_catalog::ScalarComparison::LessOrEqual => "le",
        conduit_std_catalog::ScalarComparison::Equal => "eq",
        conduit_std_catalog::ScalarComparison::NotEqual => "ne",
        conduit_std_catalog::ScalarComparison::GreaterOrEqual => "ge",
        conduit_std_catalog::ScalarComparison::Greater => "gt",
    };
    let source = format!(
        "form 0\n\nlogic_multi {{\n left: {LEFT_KIND}\n right: {RIGHT_KIND}\n compare: logic/compare\n compare.operator = \"{operator}\"\n when_false: {FALSE_KIND}\n when_true: {TRUE_KIND}\n select: logic/select\n sink: {SINK_KIND}\n left.value -> compare.left\n right.value -> compare.right\n compare.out -> select.selector\n when_false.value -> select.when-false\n when_true.value -> select.when-true\n select.out -> sink.value\n}}\n"
    );
    let form = parse(&source, &catalog).map_err(|_| LogicMultiError::Form)?;
    let advertisement = advertisement(host, boot, sources);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| LogicMultiError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
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
    .map_err(|_| LogicMultiError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(LogicMultiError::Plan);
    }
    Ok(PreparedLogicMulti {
        advertisement,
        plan,
        left,
        right,
        when_false,
        when_true,
        comparison,
    })
}

fn advertisement(
    host: &str,
    boot: &str,
    sources: [(&'static str, Scalar); 4],
) -> HostAdvertisement {
    let mut capabilities = sources
        .into_iter()
        .map(|(kind, value)| source_offer(kind, value))
        .collect::<Vec<_>>();
    capabilities.extend([
        conduit_std_catalog::conduitos_logic_compare_scalar_offer(),
        conduit_std_catalog::conduitos_logic_select_scalar_offer(),
        sink_offer(),
    ]);
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

fn source_offer(kind: &str, value: Scalar) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!(
            "conduitos-fixture-{}-{}@1",
            kind.rsplit('/').next().unwrap_or("logic"),
            value.raw_microunits()
        )),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_LOGIC_NOT_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from("conduitos.fixture/logic-source@1"),
            artifact_id: ArtifactId::from(FIXTURE_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: limits(),
    }
}

fn sink_offer() -> CapabilityOffer {
    let mut offer = source_offer(SINK_KIND, Scalar::ZERO);
    offer.capability_id = CapabilityId::from("conduitos-fixture-logic-sink@1");
    offer.kind_contract_revision = KindContractRevision::from(SINK_REVISION);
    offer.implementation.implementation_id =
        ImplementationId::from("conduitos.fixture/logic-sink@1");
    offer.inputs = vec![PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
    }];
    offer.outputs.clear();
    offer.host_operations = vec![conduit_core::HostOperationRequirement {
        contract_id: conduit_core::HostOperationContractId::from(SINK_HOST_OPERATION),
        target_kind: Some(kind_id(SINK_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        maximum_output_bytes: 0,
    }];
    offer
}

fn limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
    }
}

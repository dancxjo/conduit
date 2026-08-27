//! One ordinary bounded Form covering portable count, toggle, and key-event fan-out.

use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId, KeyEvent,
    KindContractRevision, OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection,
    PortTemporal, kind_id, port_id,
};
use conduit_form::{ProfileCatalog, StartupCatalog, parse};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use super::portable_state_input_play::PortableStateInputError;

pub(super) const TICK_SOURCE_KIND: &str = "conduitos/fixture-tick-source";
pub(super) const KEY_SOURCE_KIND: &str = "conduitos/fixture-key-source";
pub(super) const COUNT_SINK_KIND: &str = "conduitos/fixture-count-sink";
pub(super) const BOOL_SINK_KIND: &str = "conduitos/fixture-bool-sink";
pub(super) const TEXT_KEY_SINK_KIND: &str = "conduitos/fixture-text-key-sink";
pub(super) const CHORD_KEY_SINK_KIND: &str = "conduitos/fixture-chord-key-sink";
pub(super) const CAPTURE_OPERATION: &str = "conduitos.fixture/capture-portable-state-input@1";

const SOURCE_REVISION: &str = "conduitos/fixture-portable-source@1";
const SINK_REVISION: &str = "conduitos/fixture-portable-sink@1";
const FIXTURE_ARTIFACT: &str = "conduitos/portable-state-input-fixture@1";

pub struct PreparedPortableStateInput {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub(super) tick_sequence: u64,
    pub(super) count_start: u64,
    pub(super) toggle_initial: bool,
    pub(super) key: KeyEvent,
}

pub fn prepare_portable_state_input(
    host: &str,
    boot: &str,
    count_start: u64,
    toggle_initial: bool,
    key: KeyEvent,
) -> Result<PreparedPortableStateInput, PortableStateInputError> {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_count_pipeline_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PortableStateInputError::Catalog)?;
    conduit_std_catalog::install_state_toggle_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PortableStateInputError::Catalog)?;
    conduit_std_catalog::install_input_semantic_catalogs(&mut startup, &mut catalog)
        .map_err(|_| PortableStateInputError::Catalog)?;
    let fixtures = fixture_offers();
    for offer in &fixtures {
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: offer.kind_id.clone(),
                kind_contract_revision: offer.kind_contract_revision.clone(),
                inputs: offer.inputs.clone(),
                outputs: offer.outputs.clone(),
                configuration: Vec::new(),
            })
            .map_err(|_| PortableStateInputError::Catalog)?;
    }
    let source = format!(
        "form portable_state_input {{\n count_tick: {TICK_SOURCE_KIND}\n count: state/count(start = {count_start})\n count_sink: {COUNT_SINK_KIND}\n toggle_tick: {TICK_SOURCE_KIND}\n toggle: state/toggle(initial = {toggle_initial})\n bool_sink: {BOOL_SINK_KIND}\n key_source: {KEY_SOURCE_KIND}\n split: input/key-tee\n text_sink: {TEXT_KEY_SINK_KIND}\n chord_sink: {CHORD_KEY_SINK_KIND}\n count_tick.tick > count.bump\n count.value > count_sink.value\n toggle_tick.tick > toggle.toggle\n toggle.value > bool_sink.value\n key_source.key > split.key\n split.text-keys > text_sink.key\n split.chord-keys > chord_sink.key\n}}\n"
    );
    let form = parse(&source, &catalog).map_err(|_| PortableStateInputError::Form)?;
    let tick_sequence = 1;
    let advertisement = advertisement(host, boot, fixtures);
    let hosts = [advertisement.clone()];
    let placements =
        default_placements(&form, &hosts).map_err(|_| PortableStateInputError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_std_catalog::COUNT_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PortableStateInputError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(PortableStateInputError::Plan);
    }
    Ok(PreparedPortableStateInput {
        advertisement,
        plan,
        tick_sequence,
        count_start,
        toggle_initial,
        key,
    })
}

fn advertisement(host: &str, boot: &str, fixtures: Vec<CapabilityOffer>) -> HostAdvertisement {
    let mut capabilities = vec![
        conduit_std_catalog::conduitos_state_count_offer(),
        conduit_std_catalog::conduitos_state_toggle_offer(),
        conduit_std_catalog::conduitos_key_event_tee_offer(),
    ];
    capabilities.extend(fixtures);
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(conduit_std_catalog::CONDUITOS_PORTABLE_STATE_INPUT_PROFILE),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

fn fixture_offers() -> Vec<CapabilityOffer> {
    vec![
        source_offer(
            TICK_SOURCE_KIND,
            conduit_time::TICK_VALUE_KIND,
            "tick",
            conduit_time::TICK_ENCODED_LEN,
        ),
        source_offer(
            KEY_SOURCE_KIND,
            conduit_core::KEY_EVENT_INFO_ID,
            "key",
            conduit_core::KEY_EVENT_ENCODED_LEN as u32,
        ),
        sink_offer(
            COUNT_SINK_KIND,
            conduit_std_catalog::STATE_COUNT_VALUE_KIND,
            "value",
            conduit_std_catalog::COUNT_ENCODED_LEN,
            PortTemporal::Current,
        ),
        sink_offer(
            BOOL_SINK_KIND,
            conduit_core::BOOL_INFO_ID,
            "value",
            conduit_core::BOOL_ENCODED_LEN as u32,
            PortTemporal::Current,
        ),
        sink_offer(
            TEXT_KEY_SINK_KIND,
            conduit_core::KEY_EVENT_INFO_ID,
            "key",
            conduit_core::KEY_EVENT_ENCODED_LEN as u32,
            PortTemporal::Flow { closes: true },
        ),
        sink_offer(
            CHORD_KEY_SINK_KIND,
            conduit_core::KEY_EVENT_INFO_ID,
            "key",
            conduit_core::KEY_EVENT_ENCODED_LEN as u32,
            PortTemporal::Flow { closes: true },
        ),
    ]
}

fn source_offer(kind: &str, value_kind: &str, port: &str, maximum_bytes: u32) -> CapabilityOffer {
    fixture_offer(
        kind,
        SOURCE_REVISION,
        Vec::new(),
        vec![port_descriptor(
            port,
            value_kind,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
        Vec::new(),
        maximum_bytes,
    )
}

fn sink_offer(
    kind: &str,
    value_kind: &str,
    port: &str,
    maximum_bytes: u32,
    temporal: PortTemporal,
) -> CapabilityOffer {
    fixture_offer(
        kind,
        SINK_REVISION,
        vec![port_descriptor(
            port,
            value_kind,
            PortDirection::Input,
            temporal,
        )],
        Vec::new(),
        vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(CAPTURE_OPERATION),
            target_kind: Some(kind_id(kind)),
            maximum_in_flight: 1,
            maximum_input_bytes: maximum_bytes,
            maximum_output_bytes: 0,
        }],
        maximum_bytes,
    )
}

fn fixture_offer(
    kind: &str,
    revision: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    host_operations: Vec<conduit_core::HostOperationRequirement>,
    maximum_bytes: u32,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("{}-capability@1", kind.replace('/', "-"))),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_PORTABLE_STATE_INPUT_PROFILE,
            ),
            implementation_id: ImplementationId::from(format!("{kind}@1")),
            artifact_id: ArtifactId::from(FIXTURE_ARTIFACT),
        },
        inputs,
        outputs,
        host_operations,
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 2,
            max_queue_bytes: maximum_bytes.saturating_mul(2).max(16),
        },
    }
}

fn port_descriptor(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
    }
}

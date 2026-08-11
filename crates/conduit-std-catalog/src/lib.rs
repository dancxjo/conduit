#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, CapabilityLimits, ConfigurationValue, ExecutionProfileId,
    HostOperationRequirement, KindContractRevision, KindId, PortDescriptor, PortDirection,
    ResourceOffer, ResourceRequirement, PRESENTATION_RESOURCE_CLASS, TIMER_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

mod functional_face;
mod keyboard;
pub use keyboard::*;
mod palette_metadata;
mod tick;
use functional_face::startup_face;
pub use palette_metadata::*;
pub use tick::*;
mod tick_presentation;
pub use tick_presentation::*;
mod presentation_bool;
pub use presentation_bool::*;
mod presentation_composition;
pub use presentation_composition::*;
mod graphics;
pub use graphics::*;
mod time_every;
pub use time_every::*;
mod timing;
pub use timing::*;
mod text_presentation;
pub use text_presentation::*;
mod text_transform;
pub use text_transform::*;
mod state_count;
pub use state_count::*;
mod state_toggle;
pub use state_toggle::*;
mod flow_state;
pub use flow_state::*;
mod logic;
pub use logic::*;
mod math;
pub use math::*;
mod layout;
pub use layout::*;
mod patchbay_presentation;
pub use patchbay_presentation::*;
mod robotics;
pub use robotics::*;
#[cfg(feature = "form-catalog")]
mod robotics_catalog;
#[cfg(feature = "form-catalog")]
pub use robotics_catalog::install_robotics_catalogs;
mod copy_file;
pub use copy_file::*;
mod sound;
pub use sound::*;
mod sound_compatibility;
pub use sound_compatibility::*;
mod sound_stream;
pub use sound_stream::*;
#[cfg(feature = "form-catalog")]
mod sound_catalog;
#[cfg(feature = "form-catalog")]
pub use sound_catalog::install_sound_catalogs;

/// Exact typed contracts currently supported by the executable `conduit.std` nucleus.
///
/// This deliberately excludes the eight legacy contracts returned by
/// [`standard_contracts`]. Those revisions use `value/any` and remain audited
/// compatibility fixtures rather than supported operations.
pub fn supported_nucleus_contracts() -> Vec<StandardKindContract> {
    vec![
        tick_contract(),
        time_every_contract(),
        time_debounce_contract(),
        time_timeout_contract(),
        time_delay_contract(),
        time_throttle_contract(),
        tick_presentation_contract(),
        bool_presentation_contract(),
        text_literal_contract(),
        text_upper_contract(),
        text_join_contract(),
        text_presentation_contract(),
        state_count_contract(),
        state_toggle_contract(),
        count_presentation_contract(),
        state_latest_scalar_contract(),
        flow_tee_scalar_contract(),
        flow_gate_scalar_contract(),
        logic_compare_scalar_contract(),
        logic_not_contract(),
        logic_select_scalar_contract(),
        math_clamp_contract(),
        math_scale_contract(),
        math_deadband_contract(),
        layout_viewport_contract(),
        layout_inset_contract(),
        layout_row_contract(),
        layout_column_contract(),
        layout_stack_contract(),
        layout_align_contract(),
        presentation_icon_contract(),
        presentation_frame_contract(),
        presentation_badge_contract(),
        graphics_rect_contract(),
        graphics_text_contract(),
        graphics_icon_contract(),
        patchbay_presentation_contracts()[0].clone(),
        patchbay_presentation_contracts()[1].clone(),
        patchbay_presentation_contracts()[2].clone(),
        patchbay_presentation_contracts()[3].clone(),
        robotics_observe_bump_contract(),
        robotics_observe_imu_contract(),
        robotics_observe_range_contract(),
        robotics_observe_odometry_contract(),
        robotics_observe_battery_contract(),
        robotics_velocity_intent_contract(),
        robotics_drive_differential_contract(),
        copy_file_contract(),
    ]
}

/// One exact accepted implementation offer corresponding to each supported contract.
///
/// These values include the revision, implementation, artifact, resource,
/// host-operation, and finite-limit facts that an immutable Plan seals after
/// checked-face compatibility and admission filtering.
pub fn supported_nucleus_offers() -> Vec<conduit_core::CapabilityOffer> {
    vec![
        tick_capability_offer(),
        time_every_offer(),
        time_debounce_offer(),
        time_timeout_offer(),
        time_delay_offer(),
        time_throttle_offer(),
        tick_presentation_offer(),
        bool_presentation_browser_offer(),
        text_literal_offer(),
        text_upper_offer(),
        text_join_offer(),
        text_presentation_offer(),
        state_count_offer(),
        state_toggle_offer(),
        count_presentation_offer(),
        state_latest_scalar_offer(),
        flow_tee_scalar_offer(),
        flow_gate_scalar_offer(),
        logic_compare_scalar_offer(),
        logic_not_offer(),
        logic_select_scalar_offer(),
        math_clamp_offer(),
        math_scale_offer(),
        math_deadband_offer(),
        layout_viewport_offer(),
        layout_inset_offer(),
        layout_row_offer(),
        layout_column_offer(),
        layout_stack_offer(),
        layout_align_offer(),
        presentation_icon_offer(),
        presentation_frame_offer(),
        presentation_badge_offer(),
        graphics_rect_offer(),
        graphics_text_offer(),
        graphics_icon_offer(),
        patchbay_presentation_offers()[0].clone(),
        patchbay_presentation_offers()[1].clone(),
        patchbay_presentation_offers()[2].clone(),
        patchbay_presentation_offers()[3].clone(),
        robotics_observe_bump_offer(),
        robotics_observe_imu_offer(),
        robotics_observe_range_offer(),
        robotics_observe_odometry_offer(),
        robotics_observe_battery_offer(),
        robotics_velocity_intent_offer(),
        robotics_drive_differential_offer(),
        copy_file_offer(),
    ]
}

pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const MAP_KIND: &str = "flow/map";
pub const FILTER_KIND: &str = "flow/filter";
pub const TEE_KIND: &str = "flow/tee";
pub const GATE_KIND: &str = "flow/gate";
pub const FORMAT_KIND: &str = "text/format";
pub const TICK_KIND: &str = "time/tick";
pub const LATEST_KIND: &str = "state/latest";

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
pub const GENERIC_VALUE_KIND: &str = "value/any";
pub const TEXT_VALUE_KIND: &str = "value/text";

pub const IN_PORT: &str = "in";
pub const OUT_PORT: &str = "out";
pub const SIGNAL_PORT: &str = "signal";
pub const TEXT_PORT: &str = "text";
pub const TICK_PORT: &str = "tick";
pub const LEFT_PORT: &str = "left";
pub const RIGHT_PORT: &str = "right";
pub const ENABLE_PORT: &str = "enable";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalBehavior {
    EmitsOnce,
    CompletesAfterConfiguredCount,
    CompletesAfterFixedCount { count: u64 },
    CompletesWhenInputsClose,
    MirrorsInputTerminal,
    RetainsLatestUntilReleased,
    EmitsCurrentAndCompletesWhenInputCloses,
    CoupledAtomicFanoutAndMirrorsInputTerminal,
    CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose,
    EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
    TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
    InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
    DelaysEachValueInOrderAndDrainsOnInputClosure,
    LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
    SimulatedCurrentObservationEmitsOnce,
    SimulatedDriveProjectionCompletesWhenInputsClose,
    HostInputEndsOrFailsSource,
    EmitsInitialAndTogglesUntilInputCloses,
}

/// User-facing semantic contracts, including portable Kinds without a currently
/// installed std implementation. This is discovery truth, not a Host offer.
pub fn palette_contracts() -> Vec<StandardKindContract> {
    let mut contracts = supported_nucleus_contracts();
    contracts.push(keyboard_contract());
    contracts
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardConfigurationField {
    pub key: String,
    pub default_value: ConfigurationValue,
    pub rule: StandardConfigurationRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StandardConfigurationRule {
    Any,
    U64Range { minimum: u64, maximum: u64 },
    I64Range { minimum: i64, maximum: i64 },
    DurationMillis { minimum: u64, maximum: u64 },
    TextBytes { maximum: u32 },
    TextOneOf { values: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardKindContract {
    pub kind_id: KindId,
    pub plain_name: String,
    pub summary: String,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<StandardConfigurationField>,
    pub limits: CapabilityLimits,
    pub terminal_behavior: TerminalBehavior,
    pub hosted_implementation_required: bool,
    pub browser_manifestation_honest: bool,
    pub pico_manifestation_honest: bool,
    pub example: String,
}

pub fn standard_contracts() -> Vec<StandardKindContract> {
    vec![
        StandardKindContract {
            kind_id: kind_id(PULSE_KIND),
            plain_name: "Pulse".to_string(),
            summary: "Emit a bounded alternating signal sequence.".to_string(),
            inputs: Vec::new(),
            outputs: vec![port(SIGNAL_PORT, GENERIC_VALUE_KIND, PortDirection::Output)],
            configuration: vec![
                u64_field("count", 16, 0, 4_096),
                u64_field("period-ms", 250, 0, u64::MAX),
                bool_field("initial", false),
            ],
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::CompletesAfterConfiguredCount,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "pulse: flow/pulse".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(SHOW_KIND),
            plain_name: "Show".to_string(),
            summary: "Present each input value through a host-honest manifestation.".to_string(),
            inputs: vec![port(SIGNAL_PORT, GENERIC_VALUE_KIND, PortDirection::Input)],
            outputs: Vec::new(),
            configuration: Vec::new(),
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "show: presentation/show".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(MAP_KIND),
            plain_name: "Map".to_string(),
            summary: "Transform one bounded input stream into one bounded output stream."
                .to_string(),
            inputs: vec![port(IN_PORT, GENERIC_VALUE_KIND, PortDirection::Input)],
            outputs: vec![port(OUT_PORT, GENERIC_VALUE_KIND, PortDirection::Output)],
            configuration: vec![u64_field("function-id", 0, 0, u64::MAX)],
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "mapped: flow/map".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(FILTER_KIND),
            plain_name: "Filter".to_string(),
            summary: "Forward only values accepted by a bounded predicate.".to_string(),
            inputs: vec![port(IN_PORT, GENERIC_VALUE_KIND, PortDirection::Input)],
            outputs: vec![port(OUT_PORT, GENERIC_VALUE_KIND, PortDirection::Output)],
            configuration: vec![u64_field("predicate-id", 0, 0, u64::MAX)],
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "filtered: flow/filter".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(TEE_KIND),
            plain_name: "Tee".to_string(),
            summary: "Copy each input value to two bounded output branches.".to_string(),
            inputs: vec![port(IN_PORT, GENERIC_VALUE_KIND, PortDirection::Input)],
            outputs: vec![
                port(LEFT_PORT, GENERIC_VALUE_KIND, PortDirection::Output),
                port(RIGHT_PORT, GENERIC_VALUE_KIND, PortDirection::Output),
            ],
            configuration: Vec::new(),
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "split: flow/tee".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(FORMAT_KIND),
            plain_name: "Format text".to_string(),
            summary: "Render each input value into a bounded text value.".to_string(),
            inputs: vec![port(IN_PORT, GENERIC_VALUE_KIND, PortDirection::Input)],
            outputs: vec![port(TEXT_PORT, GENERIC_VALUE_KIND, PortDirection::Output)],
            configuration: vec![u64_field("template-id", 0, 0, u64::MAX)],
            limits: limits(16, 4, 256),
            terminal_behavior: TerminalBehavior::CompletesWhenInputsClose,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "formatted: text/format".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(TICK_KIND),
            plain_name: "Tick".to_string(),
            summary: "Emit a bounded timer tick sequence.".to_string(),
            inputs: Vec::new(),
            outputs: vec![port(TICK_PORT, GENERIC_VALUE_KIND, PortDirection::Output)],
            configuration: vec![
                u64_field("count", 16, 0, 4_096),
                u64_field("period-ms", 1_000, 0, u64::MAX),
            ],
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::CompletesAfterConfiguredCount,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "clock: time/tick".to_string(),
        },
        StandardKindContract {
            kind_id: kind_id(LATEST_KIND),
            plain_name: "Latest state".to_string(),
            summary: "Retain the latest input value and emit bounded updates.".to_string(),
            inputs: vec![port(IN_PORT, GENERIC_VALUE_KIND, PortDirection::Input)],
            outputs: vec![port(OUT_PORT, GENERIC_VALUE_KIND, PortDirection::Output)],
            configuration: Vec::new(),
            limits: limits(16, 4, 64),
            terminal_behavior: TerminalBehavior::RetainsLatestUntilReleased,
            hosted_implementation_required: true,
            browser_manifestation_honest: false,
            pico_manifestation_honest: false,
            example: "latest: state/latest".to_string(),
        },
    ]
}

#[cfg(test)]
mod supported_nucleus_tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    fn supported_nucleus_is_typed_hosted_and_identity_unique() {
        let contracts = supported_nucleus_contracts();
        let offers = supported_nucleus_offers();
        assert_eq!(contracts.len(), 48);
        assert_eq!(offers.len(), contracts.len());

        let identities = contracts
            .iter()
            .map(|contract| contract.kind_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), contracts.len());

        for contract in &contracts {
            assert!(contract.hosted_implementation_required);
            assert_eq!(
                contract.browser_manifestation_honest,
                matches!(
                    contract.kind_id.as_str(),
                    BOOL_PRESENTATION_KIND | PATCHBAY_PRESENTATION_KIND
                )
            );
            assert!(!contract.pico_manifestation_honest);
            assert!(contract
                .inputs
                .iter()
                .chain(contract.outputs.iter())
                .all(|port| port.value_kind.as_str() != GENERIC_VALUE_KIND));
        }

        for (contract, offer) in contracts.iter().zip(&offers) {
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
        }

        let offer_identities = offers
            .iter()
            .map(|offer| {
                (
                    offer.kind_contract_revision.as_str(),
                    offer.implementation.implementation_id.as_str(),
                    offer.implementation.artifact_id.as_str(),
                )
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(offer_identities.len(), offers.len());
    }
}

pub fn find_contract(kind: &KindId) -> Option<StandardKindContract> {
    standard_contracts()
        .into_iter()
        .find(|contract| &contract.kind_id == kind)
}

pub fn standard_kind_ids() -> Vec<KindId> {
    standard_contracts()
        .into_iter()
        .map(|contract| contract.kind_id)
        .collect()
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: conduit_core::PortTemporal::Value,
    }
}

fn limits(
    max_active_instances: u16,
    max_queue_items: u16,
    max_queue_bytes: u32,
) -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances,
        max_queue_items,
        max_queue_bytes,
    }
}

fn contract_revision(kind: &KindId) -> KindContractRevision {
    KindContractRevision::from(alloc::format!(
        "conduit.std/{}@1",
        capability_slug(kind.as_str())
    ))
}

fn execution_profile(kind: &KindId) -> ExecutionProfileId {
    ExecutionProfileId::from(alloc::format!(
        "conduit.std/{}-hosted@1",
        capability_slug(kind.as_str())
    ))
}

fn u64_field(
    key: &str,
    default_value: u64,
    minimum: u64,
    maximum: u64,
) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default_value),
        rule: StandardConfigurationRule::U64Range { minimum, maximum },
    }
}

fn bool_field(key: &str, default_value: bool) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Bool(default_value),
        rule: StandardConfigurationRule::Any,
    }
}

#[cfg(feature = "form-catalog")]
pub fn standard_profile_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

    let mut catalog = ProfileCatalog::new();
    for contract in standard_contracts() {
        catalog
            .insert(KindDefinition {
                kind_contract_revision: contract_revision(&contract.kind_id),
                kind_id: contract.kind_id,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| ConfigurationField {
                        key: field.key,
                        default_value: field.default_value,
                        validation: match field.rule {
                            StandardConfigurationRule::Any => ConfigurationRule::Any,
                            StandardConfigurationRule::U64Range { minimum, maximum } => {
                                ConfigurationRule::U64Range { minimum, maximum }
                            }
                            StandardConfigurationRule::I64Range { minimum, maximum } => {
                                ConfigurationRule::I64Range { minimum, maximum }
                            }
                            StandardConfigurationRule::DurationMillis { minimum, maximum } => {
                                ConfigurationRule::DurationMillis { minimum, maximum }
                            }
                            StandardConfigurationRule::TextBytes { maximum } => {
                                ConfigurationRule::TextBytes { maximum }
                            }
                            StandardConfigurationRule::TextOneOf { values } => {
                                ConfigurationRule::TextOneOf { values }
                            }
                        },
                    })
                    .collect(),
            })
            .expect("standard catalog kinds are unique");
    }
    catalog
}

pub fn standard_capability_offers(
    implementation_prefix: &str,
) -> Vec<conduit_core::CapabilityOffer> {
    standard_contracts()
        .into_iter()
        .map(|contract| conduit_core::CapabilityOffer {
            startup_parameters: startup_face(&contract.configuration),
            shorthand: None,
            capability_id: conduit_core::CapabilityId::from(capability_slug(
                contract.kind_id.as_str(),
            )),
            kind_id: contract.kind_id.clone(),
            kind_contract_revision: contract_revision(&contract.kind_id),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: execution_profile(&contract.kind_id),
                implementation_id: conduit_core::ImplementationId::from(alloc::format!(
                    "{implementation_prefix}/{}-v1",
                    capability_slug(contract.kind_id.as_str())
                )),
                artifact_id: conduit_core::ArtifactId::from(alloc::format!(
                    "conduit-std-catalog/{}",
                    capability_slug(contract.kind_id.as_str())
                )),
            },
            inputs: contract.inputs.clone(),
            outputs: contract.outputs.clone(),
            host_operations: standard_host_operation_requirements(
                &contract.kind_id,
                contract.limits.max_queue_bytes,
            ),
            resource_requirements: standard_resource_requirements(&contract.kind_id),
            authority_requirements: Vec::new(),
            limits: contract.limits,
        })
        .collect()
}

pub fn standard_host_operation_requirements(
    operation_kind: &KindId,
    maximum_value_bytes: u32,
) -> Vec<HostOperationRequirement> {
    match operation_kind.as_str() {
        PULSE_KIND | TICK_KIND => vec![wait_host_operation_requirement()],
        SHOW_KIND => vec![present_host_operation_requirement(
            kind_id("presentation/stdout"),
            maximum_value_bytes,
        )],
        _ => Vec::new(),
    }
}

pub fn standard_resource_requirements(kind_id: &KindId) -> Vec<ResourceRequirement> {
    match kind_id.as_str() {
        PULSE_KIND | TICK_KIND => vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        SHOW_KIND => vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)],
        _ => Vec::new(),
    }
}

pub fn standard_resource_offers(capacity_units: u32) -> Vec<ResourceOffer> {
    vec![
        resource_offer(
            "std-catalog/presentation",
            PRESENTATION_RESOURCE_CLASS,
            capacity_units,
        ),
        resource_offer("std-catalog/timer", TIMER_RESOURCE_CLASS, capacity_units),
    ]
}

pub fn standard_host_advertisement(
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    offer_generation: conduit_core::OfferGeneration,
) -> conduit_core::HostAdvertisement {
    conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation,
        profile: conduit_core::HostProfileId::from("conduit.std/hosted-v1"),
        resources: standard_resource_offers(16),
        planner_capabilities: vec![],
        capabilities: standard_capability_offers("std"),
    }
}

fn capability_slug(kind: &str) -> String {
    kind.replace('/', "-")
}

#[cfg(feature = "compatibility-fixture")]
mod host_profile;

#[cfg(feature = "compatibility-fixture")]
pub use host_profile::{install_standard_profile, standard_registry};

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{
        contract_revision, execution_profile, find_contract, standard_contracts,
        standard_host_advertisement, standard_host_operation_requirements,
        standard_profile_catalog, standard_registry, standard_resource_offers,
        standard_resource_requirements, startup_face, FILTER_KIND, FORMAT_KIND, GENERIC_VALUE_KIND,
        LATEST_KIND, MAP_KIND, PULSE_KIND, SHOW_KIND, TEE_KIND, TICK_KIND,
    };
    use conduit_core::{
        kind_id, ArtifactId, CapabilityId, CapabilityOffer, ConnectionBase, HostAdvertisement,
        HostCommand, HostEvent, HostId, HostProfileId, ImplementationId, ObservationKind,
        OfferGeneration, PlatformEffect, PROTOCOL_VERSION,
    };
    use conduit_form::parse;
    use conduit_planner::{plan, PlacementChoice, PlacementChoices};
    use conduit_runtime::HostRuntime;

    #[test]
    fn standard_catalog_contains_the_m4_socket_set() {
        let contracts = standard_contracts();
        let kind_ids = contracts
            .iter()
            .map(|contract| contract.kind_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            kind_ids,
            vec![
                PULSE_KIND,
                SHOW_KIND,
                MAP_KIND,
                FILTER_KIND,
                TEE_KIND,
                FORMAT_KIND,
                TICK_KIND,
                LATEST_KIND
            ]
        );
        for contract in &contracts {
            assert!(!contract.plain_name.is_empty());
            assert!(!contract.summary.is_empty());
            assert!(!contract.example.is_empty());
            assert!(contract.limits.max_active_instances > 0);
            assert!(contract.limits.max_queue_items > 0);
            assert!(contract.limits.max_queue_bytes > 0);
        }
        let map = find_contract(&kind_id(MAP_KIND)).expect("map contract exists");
        assert!(map
            .inputs
            .iter()
            .chain(map.outputs.iter())
            .all(|port| port.value_kind == kind_id(GENERIC_VALUE_KIND)));
    }

    #[test]
    fn contracts_convert_to_form_catalog_without_runtime_kind_changes() {
        let catalog = standard_profile_catalog();
        let form = parse(
            "form 0\n\nstd_catalog {\n pulse: flow/pulse\n show: presentation/show\n pulse > show\n}\n",
            &catalog,
        )
        .expect("existing pulse/show form parses through standard catalog");
        assert_eq!(form.gears.len(), 2);
        assert_eq!(form.connections.len(), 1);

        let flow_form = parse(
            "form 0\n\nstd_flow {\n clock: time/tick\n source: flow/map\n filtered: flow/filter\n split: flow/tee\n latest: state/latest\n formatted: text/format\n clock.tick -> source.in\n source > filtered\n filtered > split\n split.left -> latest.in\n split.right -> formatted.in\n}\n",
            &catalog,
        )
        .expect("new standard flow form parses");
        assert_eq!(flow_form.gears.len(), 6);
        assert_eq!(flow_form.connections.len(), 5);
    }

    #[test]
    fn conformance_fixture_plans_standard_contracts_without_ui() {
        let catalog = standard_profile_catalog();
        let form = parse(
            "form 0\n\nstd_conformance {\n clock: time/tick\n source: flow/map\n filter: flow/filter\n split: flow/tee\n latest: state/latest\n format: text/format\n clock.tick -> source.in\n source > filter\n filter > split\n split.left -> latest.in\n split.right -> format.in\n}\n",
            &catalog,
        )
        .expect("standard conformance form parses");
        let host = conformance_host_advertisement();
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([
                ("source", "flow-map"),
                ("filter", "flow-filter"),
                ("split", "flow-tee"),
                ("latest", "state-latest"),
                ("format", "text-format"),
                ("clock", "time-tick"),
            ])
            .into_iter()
            .map(|(operation, capability)| {
                (
                    conduit_core::GearId::from(operation),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: CapabilityId::from(capability),
                    },
                )
            })
            .collect(),
        };
        let plan = plan(
            &form,
            core::slice::from_ref(&host),
            &placements,
            &[ConnectionBase::Local],
        )
        .expect("standard conformance form plans");
        assert_eq!(plan.fragments.len(), 1);
        let fragment = &plan.fragments[0];
        assert_eq!(fragment.placements.len(), 6);
        assert_eq!(fragment.connections.len(), 5);
        assert!(fragment
            .placements
            .iter()
            .all(|placement| placement.implementation_id.as_str().starts_with("std/")));
    }

    #[test]
    fn hosted_standard_profile_runs_bounded_flow_form_without_ui() {
        let observations = run_hosted_standard_form(
            "form 0\n\nstd_exec {\n clock: time/tick\n map: flow/map\n filter: flow/filter\n split: flow/tee\n latest: state/latest\n format: text/format\n show_latest: presentation/show\n show_text: presentation/show\n clock.count = 1\n clock.period-ms = 0\n clock.tick -> map.in\n map > filter\n filter > split\n split.left -> latest.in\n split.right -> format.in\n latest > show_latest\n format.text -> show_text.signal\n}\n",
            [
                ("clock", "time-tick"),
                ("map", "flow-map"),
                ("filter", "flow-filter"),
                ("split", "flow-tee"),
                ("latest", "state-latest"),
                ("format", "text-format"),
                ("show_latest", "presentation-show"),
                ("show_text", "presentation-show"),
            ],
        );
        assert_completed_plan(&observations);
        assert!(
            observations
                .iter()
                .filter(|observation| matches!(
                    observation.kind,
                    ObservationKind::ValueAccepted { .. }
                ))
                .count()
                >= 5,
            "latest and format should receive values through tee branches"
        );
    }

    #[test]
    fn hosted_standard_profile_runs_pulse_show_form_without_ui() {
        let observations = run_hosted_standard_form(
            "form 0\n\nstd_pulse_show {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 1\n pulse.period-ms = 0\n pulse.signal -> show.signal\n}\n",
            [("pulse", "flow-pulse"), ("show", "presentation-show")],
        );
        assert_completed_plan(&observations);
        assert_presented_value(&observations);
    }

    #[test]
    fn hosted_standard_profile_runs_tick_format_show_form_without_ui() {
        let observations = run_hosted_standard_form(
            "form 0\n\nstd_tick_format {\n clock: time/tick\n format: text/format\n show: presentation/show\n clock.count = 1\n clock.period-ms = 0\n clock.tick -> format.in\n format.text -> show.signal\n}\n",
            [
                ("clock", "time-tick"),
                ("format", "text-format"),
                ("show", "presentation-show"),
            ],
        );
        assert_completed_plan(&observations);
        assert!(observations.iter().any(|observation| {
            matches!(
                &observation.kind,
                ObservationKind::ValuePresented { value }
                    if value.encoded.as_slice() == b"value:0"
            )
        }));
    }

    #[test]
    fn platform_manifestation_truth_is_explicit() {
        let contracts = standard_contracts();
        for contract in contracts {
            assert!(!contract.browser_manifestation_honest);
            assert!(!contract.pico_manifestation_honest);
        }
    }

    fn conformance_host_advertisement() -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("std-catalog-host"),
            boot_id: conduit_core::BootId::from("std-catalog-boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("conduit.std/conformance"),
            resources: standard_resource_offers(16),
            planner_capabilities: vec![],
            capabilities: vec![
                offer("flow-pulse", PULSE_KIND, "std/pulse-v1"),
                offer("presentation-show", SHOW_KIND, "std/show-v1"),
                offer("flow-map", MAP_KIND, "std/map-v1"),
                offer("flow-filter", FILTER_KIND, "std/filter-v1"),
                offer("flow-tee", TEE_KIND, "std/tee-v1"),
                offer("text-format", FORMAT_KIND, "std/text-format-v1"),
                offer("time-tick", TICK_KIND, "std/time-tick-v1"),
                offer("state-latest", LATEST_KIND, "std/latest-v1"),
            ],
        }
    }

    fn offer(capability: &str, kind: &str, implementation: &str) -> CapabilityOffer {
        let kind_id = kind_id(kind);
        let contract = find_contract(&kind_id).expect("standard contract exists");
        let startup_parameters = startup_face(&contract.configuration);
        CapabilityOffer {
            startup_parameters,
            shorthand: None,
            capability_id: CapabilityId::from(capability),
            kind_id: kind_id.clone(),
            kind_contract_revision: contract_revision(&kind_id),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: execution_profile(&kind_id),
                implementation_id: ImplementationId::from(implementation),
                artifact_id: ArtifactId::from(
                    alloc::format!("conduit-std-catalog/{kind}").as_str(),
                ),
            },
            inputs: contract.inputs,
            outputs: contract.outputs,
            host_operations: standard_host_operation_requirements(
                &kind_id,
                contract.limits.max_queue_bytes,
            ),
            resource_requirements: standard_resource_requirements(&kind_id),
            authority_requirements: vec![],
            limits: conduit_core::CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        }
    }

    fn placements_for<const N: usize>(
        host: &HostAdvertisement,
        mappings: [(&str, &str); N],
    ) -> PlacementChoices {
        PlacementChoices {
            by_gear: mappings
                .into_iter()
                .map(|(operation, capability)| {
                    (
                        conduit_core::GearId::from(operation),
                        PlacementChoice {
                            host_id: host.host_id.clone(),
                            capability_id: CapabilityId::from(capability),
                        },
                    )
                })
                .collect(),
        }
    }

    fn run_hosted_standard_form<const N: usize>(
        form_source: &str,
        mappings: [(&str, &str); N],
    ) -> Vec<conduit_core::Observation> {
        let catalog = standard_profile_catalog();
        let form = parse(form_source, &catalog).expect("executable standard form parses");
        let host = standard_host_advertisement(
            HostId::from("std-catalog-host"),
            conduit_core::BootId::from("std-catalog-boot"),
            OfferGeneration(1),
        );
        let placements = placements_for(&host, mappings);
        let plan = plan(
            &form,
            core::slice::from_ref(&host),
            &placements,
            &[ConnectionBase::Local],
        )
        .expect("hosted standard form plans");
        let fragment = plan.fragments.first().expect("fragment exists").clone();
        let mut runtime = HostRuntime::new(
            host,
            standard_registry("std").expect("standard registry installs"),
            128,
        );
        let prepared = runtime.handle(HostCommand::Prepare(fragment.clone()));
        assert!(
            prepared.events.iter().any(|event| {
                matches!(
                    event,
                    HostEvent::Prepared { plan_id } if plan_id == &fragment.plan_id
                )
            }),
            "prepare events: {:?}",
            prepared.events
        );
        drive_runtime(&mut runtime, fragment.plan_id);
        inspect(&mut runtime)
    }

    fn assert_completed_plan(observations: &[conduit_core::Observation]) {
        assert!(
            observations.iter().any(|observation| {
                matches!(
                    observation.kind,
                    ObservationKind::PlanTerminal {
                        disposition: conduit_core::TerminalDisposition::Completed
                    }
                )
            }),
            "observations: {:?}",
            observations
        );
    }

    fn assert_presented_value(observations: &[conduit_core::Observation]) {
        assert!(observations
            .iter()
            .any(|observation| matches!(observation.kind, ObservationKind::ValuePresented { .. })));
    }

    fn drive_runtime(runtime: &mut HostRuntime, plan_id: conduit_core::PlanId) {
        let mut pending = runtime.handle(HostCommand::StartPlay(plan_id)).effects;
        while let Some(effect) = pending.pop() {
            let output = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    ..
                } => runtime.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                }),
                PlatformEffect::PresentValue {
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                    ..
                } => runtime.handle(HostCommand::CompletePresentation {
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                    success: true,
                    message: None,
                }),
                PlatformEffect::TransmitConnection { .. } => {
                    panic!("standard catalog conformance uses only local connections")
                }
            };
            pending.extend(output.effects);
        }
    }

    fn inspect(runtime: &mut HostRuntime) -> Vec<conduit_core::Observation> {
        runtime
            .handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .expect("inspect returns observations")
    }
}

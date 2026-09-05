#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, present_host_operation_requirement, resource_requirement,
    wait_host_operation_requirement, CapabilityLimits, ConfigurationValue,
    HostOperationRequirement, KindId, PortDescriptor, ResourceRequirement,
    PRESENTATION_RESOURCE_CLASS, TIMER_RESOURCE_CLASS,
};
use serde::{Deserialize, Serialize};

mod functional_face;
mod normalized_quantity;
mod quantity_info;
pub use functional_face::{realization_offer, RealizationOfferIdentity};
pub use normalized_quantity::*;
pub use quantity_info::*;
mod keyboard;
pub use keyboard::*;
mod input_semantics;
pub use input_semantics::*;
mod http;
pub use http::*;
mod resource_snapshot;
pub use resource_snapshot::*;
mod json;
pub use json::*;
mod structured_selector;
pub use structured_selector::*;
mod structured_values;
pub use structured_values::*;
mod diverse_structured_info;
pub use diverse_structured_info::*;
mod delivery_classification;
pub use delivery_classification::*;
mod vision;
pub use vision::*;
mod vision_realization;
pub use vision_realization::*;
#[cfg(feature = "form-catalog")]
mod vision_catalog;
#[cfg(feature = "form-catalog")]
pub use vision_catalog::*;
#[cfg(feature = "form-catalog")]
mod education;
#[cfg(feature = "form-catalog")]
pub use education::*;
#[cfg(feature = "form-catalog")]
mod education_realization;
#[cfg(feature = "form-catalog")]
mod education_value;
#[cfg(feature = "form-catalog")]
pub use education_realization::*;
#[cfg(feature = "form-catalog")]
mod education_catalog;
#[cfg(feature = "form-catalog")]
pub use education_catalog::*;
mod generalized_input;
pub use generalized_input::*;
mod button_indicator;
pub use button_indicator::*;
mod generalized_input_realization;
pub use generalized_input_realization::*;
#[cfg(feature = "form-catalog")]
mod generalized_input_catalog;
#[cfg(feature = "form-catalog")]
pub use generalized_input_catalog::*;
mod job;
pub use job::*;
#[cfg(feature = "form-catalog")]
mod job_catalog;
#[cfg(feature = "form-catalog")]
pub use job_catalog::*;
mod palette_metadata;
mod tick;
pub use functional_face::startup_face;
pub use palette_metadata::*;
pub use tick::*;
mod tick_presentation;
pub use tick_presentation::*;
mod presentation_bool;
pub use presentation_bool::*;
mod presentation_indicator;
pub use presentation_indicator::*;
mod presentation_composition;
pub use presentation_composition::*;
mod presentation_execution;
pub use presentation_execution::*;
mod browser_human_io;
pub use browser_human_io::*;
mod human_media_catalog;
pub use human_media_catalog::*;
#[cfg(feature = "body-coordination-plan")]
mod body_coordination_plan;
#[cfg(feature = "body-coordination-plan")]
pub use body_coordination_plan::*;
mod graphics;
pub use graphics::*;
mod graphics_presentation;
pub use graphics_presentation::*;
mod time_every;
pub use time_every::*;
mod timing;
pub use timing::*;
#[cfg(feature = "form-catalog")]
mod button_attempt_codec;
#[cfg(feature = "form-catalog")]
mod timed_interval_codec;
#[cfg(feature = "form-catalog")]
pub use button_attempt_codec::{
    BoundedButtonAttemptCodec, ButtonAttemptObservation, ButtonAttemptRefusal,
};
#[cfg(feature = "form-catalog")]
pub use timed_interval_codec::BoundedIntervalCodec;
#[cfg(feature = "form-catalog")]
mod sequence_normalization_codec;
#[cfg(feature = "form-catalog")]
pub use sequence_normalization_codec::BoundedNormalizationCodec;
#[cfg(feature = "form-catalog")]
mod timed_pattern;
#[cfg(feature = "form-catalog")]
pub use timed_pattern::*;
#[cfg(feature = "form-catalog")]
mod timed_button_attempt;
#[cfg(feature = "form-catalog")]
pub use timed_button_attempt::*;
#[cfg(feature = "form-catalog")]
mod sequence_normalization;
#[cfg(feature = "form-catalog")]
pub use sequence_normalization::*;
#[cfg(feature = "form-catalog")]
mod final_normalized_pattern;
#[cfg(feature = "form-catalog")]
pub use final_normalized_pattern::*;
#[cfg(feature = "form-catalog")]
mod pattern_comparison_codec;
#[cfg(feature = "form-catalog")]
pub use pattern_comparison_codec::{BoundedPatternComparisonCodec, PatternComparisonInput};
#[cfg(feature = "form-catalog")]
mod pattern_comparison;
#[cfg(feature = "form-catalog")]
pub use pattern_comparison::*;
#[cfg(feature = "form-catalog")]
mod template_collection;
#[cfg(feature = "form-catalog")]
pub use template_collection::*;
#[cfg(feature = "form-catalog")]
mod template_storage;
#[cfg(feature = "form-catalog")]
pub use template_storage::*;
#[cfg(feature = "form-catalog")]
mod recurrence_catalog;
#[cfg(feature = "form-catalog")]
pub use recurrence_catalog::*;
#[cfg(feature = "form-catalog")]
mod calendar_proposal_catalog;
#[cfg(feature = "form-catalog")]
pub use calendar_proposal_catalog::*;
#[cfg(feature = "form-catalog")]
mod calendar_provider_catalog;
#[cfg(feature = "form-catalog")]
pub use calendar_provider_catalog::*;
mod schedule;
pub use schedule::*;
mod scheduled_job;
pub use scheduled_job::*;
#[cfg(feature = "form-catalog")]
mod reminder_catalog;
#[cfg(feature = "form-catalog")]
pub use reminder_catalog::*;
#[cfg(feature = "form-catalog")]
mod schedule_realization;
#[cfg(feature = "form-catalog")]
pub use schedule_realization::*;
#[cfg(feature = "form-catalog")]
mod schedule_catalog;
#[cfg(feature = "form-catalog")]
pub use schedule_catalog::*;
mod text_presentation;
pub use text_presentation::*;
mod text_transform;
pub use text_transform::*;
mod value_primitives;
pub use value_primitives::*;
#[cfg(feature = "text-lab-plan")]
mod text_lab_plan;
#[cfg(feature = "text-lab-plan")]
pub use text_lab_plan::*;
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
mod quantity_mapping;
pub use quantity_mapping::*;
mod signal_garden;
pub use signal_garden::*;
#[cfg(feature = "form-catalog")]
mod signal_garden_catalog;
#[cfg(feature = "form-catalog")]
pub use signal_garden_catalog::*;
mod layout;
pub use layout::*;
mod patchbay_presentation;
pub use patchbay_presentation::*;
mod robotics;
pub use robotics::*;
mod robotics_hazard;
pub use robotics_hazard::*;
mod robotics_input;
pub use robotics_input::*;
mod robotics_structured;
pub use robotics_structured::*;
mod robotics_structured_realization;
pub use robotics_structured_realization::*;
#[cfg(feature = "form-catalog")]
mod robotics_catalog;
#[cfg(feature = "form-catalog")]
pub use robotics_catalog::install_robotics_catalogs;
#[cfg(feature = "form-catalog")]
mod robotics_structured_catalog;
#[cfg(feature = "form-catalog")]
pub use robotics_structured_catalog::*;
mod copy_file;
pub use copy_file::*;
mod sound;
pub use sound::*;
mod audio_render_demand;
pub use audio_render_demand::*;
mod alife;
pub use alife::*;
mod music_input;
pub use music_input::*;
mod sound_compatibility;
pub use sound_compatibility::*;
mod sound_stream;
pub use sound_stream::*;
#[cfg(feature = "form-catalog")]
mod sound_catalog;
#[cfg(feature = "form-catalog")]
pub use sound_catalog::install_sound_catalogs;
#[cfg(feature = "form-catalog")]
mod structured_music_form;
#[cfg(feature = "form-catalog")]
pub use structured_music_form::*;
mod supported_nucleus;
pub use supported_nucleus::*;

pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const TEE_KIND: &str = "flow/tee";
pub const GATE_KIND: &str = "flow/gate";
pub const TICK_KIND: &str = "time/tick";
pub const LATEST_KIND: &str = "state/latest";
pub const STATE_SELECT_KIND: &str = "state/select";

pub const SIGNAL_VALUE_KIND: &str = "value/signal";
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
    CurrentScalarSelectorCompletesWhenInputsClose,
    EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
    TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses,
    InactivityStateCancelsDeadlineAndCompletesWhenInputCloses,
    DelaysEachValueInOrderAndDrainsOnInputClosure,
    LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses,
    SimulatedCurrentObservationEmitsOnce,
    HostInputEndsOrFailsSource,
    HostObservationEndsOrFailsSource,
    EmitsInitialAndTogglesUntilInputCloses,
    EmitsOneField,
    EvolvesAfterTicksAndCompletesWhenTickCloses,
    PresentsEachFieldAndCompletesWhenInputCloses,
    CompletesAfterDockedRefusedOrDeadline,
}

/// User-facing semantic contracts, including portable Kinds without a currently
/// installed std implementation. This is discovery truth, not a Host offer.
pub fn palette_contracts() -> Vec<StandardKindContract> {
    let mut contracts = supported_nucleus_contracts();
    contracts.extend(patchbay_presentation_contracts());
    contracts.extend(alife_contracts());
    contracts.extend(robotics_hazard_contracts());
    contracts.push(keyboard_contract());
    contracts.extend(http_contracts());
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

#[cfg(feature = "form-catalog")]
pub fn standard_profile_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

    let mut catalog = ProfileCatalog::new();
    for (contract, revision) in supported_nucleus_contracts_with_revisions() {
        catalog
            .insert(KindDefinition {
                kind_contract_revision: conduit_core::KindContractRevision::from(revision),
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

#[cfg(test)]
mod supported_nucleus_tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    fn supported_nucleus_contracts_are_typed_and_identity_unique() {
        let contracts = supported_nucleus_contracts();
        assert_eq!(contracts.len(), 56);

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
                    conduit_time::TIME_EVERY_KIND
                        | STATE_COUNT_KIND
                        | COUNT_PRESENTATION_KIND
                        | BOOL_PRESENTATION_KIND
                        | GRAPHICS_PRESENTATION_KIND
                        | PATCHBAY_PRESENTATION_KIND
                        | TEXT_PRESENTATION_KIND
                        | LAYOUT_VIEWPORT_KIND
                        | LAYOUT_INSET_KIND
                        | LAYOUT_ROW_KIND
                        | LAYOUT_COLUMN_KIND
                        | LAYOUT_STACK_KIND
                        | LAYOUT_ALIGN_KIND
                        | PRESENTATION_ICON_KIND
                        | PRESENTATION_FRAME_KIND
                        | PRESENTATION_BADGE_KIND
                        | GRAPHICS_RECT_KIND
                        | GRAPHICS_TEXT_KIND
                        | GRAPHICS_ICON_KIND
                )
            );
            assert!(!contract.pico_manifestation_honest);
            assert!(contract
                .inputs
                .iter()
                .chain(contract.outputs.iter())
                .all(|port| port.value_kind.as_str() != "value/any"));
        }
    }

    #[cfg(feature = "form-catalog")]
    #[test]
    fn portable_profile_revisions_are_exact_without_reading_host_offers() {
        let catalog = standard_profile_catalog();

        for (contract, revision) in supported_nucleus_contracts_with_revisions() {
            let definition = catalog
                .get(&contract.kind_id)
                .expect("portable contract is present in the profile catalog");
            assert_eq!(definition.kind_contract_revision.as_str(), revision);
            assert_eq!(definition.inputs, contract.inputs);
            assert_eq!(definition.outputs, contract.outputs);
        }
    }
}

#[cfg(feature = "kernel-operation")]
mod pattern_comparison_operation;
#[cfg(feature = "kernel-operation")]
pub use pattern_comparison_operation::PatternComparisonOperation;

#[cfg(feature = "kernel-operation")]
mod template_storage_operation;
#[cfg(feature = "kernel-operation")]
pub use template_storage_operation::TemplateStorageOperation;
#[cfg(feature = "form-catalog")]
mod template_store;
#[cfg(feature = "form-catalog")]
pub use template_store::{BoundedTemplateStore, TemplateStoreRefusal};
#[cfg(feature = "kernel-operation")]
mod final_pattern_operation;
#[cfg(feature = "kernel-operation")]
pub use final_pattern_operation::FinalNormalizedPatternOperation;
#[cfg(feature = "kernel-operation")]
mod structured_selector_operation;
#[cfg(feature = "kernel-operation")]
pub use structured_selector_operation::StructuredSelectorOperation;

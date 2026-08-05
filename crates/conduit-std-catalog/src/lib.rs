#![cfg_attr(not(feature = "form-catalog"), no_std)]

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

pub const PULSE_KIND: &str = "flow/pulse";
pub const SHOW_KIND: &str = "presentation/show";
pub const MAP_KIND: &str = "flow/map";
pub const FILTER_KIND: &str = "flow/filter";
pub const TEE_KIND: &str = "flow/tee";
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalBehavior {
    CompletesAfterConfiguredCount,
    CompletesWhenInputsClose,
    MirrorsInputTerminal,
    RetainsLatestUntilReleased,
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
            browser_manifestation_honest: true,
            pico_manifestation_honest: true,
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
            browser_manifestation_honest: true,
            pico_manifestation_honest: true,
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
            capability_id: conduit_core::CapabilityId::from(capability_slug(
                contract.kind_id.as_str(),
            )),
            kind_id: contract.kind_id.clone(),
            kind_contract_revision: contract_revision(&contract.kind_id),
            execution_profile_id: execution_profile(&contract.kind_id),
            implementation_id: conduit_core::ImplementationId::from(alloc::format!(
                "{implementation_prefix}/{}-v1",
                capability_slug(contract.kind_id.as_str())
            )),
            artifact_id: conduit_core::ArtifactId::from(alloc::format!(
                "conduit-std-catalog/{}",
                capability_slug(contract.kind_id.as_str())
            )),
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
        capabilities: standard_capability_offers("std"),
    }
}

fn capability_slug(kind: &str) -> String {
    kind.replace('/', "-")
}

#[cfg(feature = "host-profile")]
mod host_profile;

#[cfg(feature = "host-profile")]
pub use host_profile::{install_standard_profile, standard_registry};

#[cfg(test)]
mod tests;

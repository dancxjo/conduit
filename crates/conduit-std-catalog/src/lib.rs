#![cfg_attr(not(feature = "form-catalog"), no_std)]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindId, PortDescriptor, PortDirection,
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
pub const TICK_VALUE_KIND: &str = "value/tick";

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
            outputs: vec![port(SIGNAL_PORT, SIGNAL_VALUE_KIND, PortDirection::Output)],
            configuration: vec![
                u64_field("count", 16, 0, 4_096),
                u64_field("period-ms", 250, 0, u64::MAX),
                bool_field("initial", false),
            ],
            limits: limits(SIGNAL_VALUE_KIND, 16, 4, 64),
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
            inputs: vec![port(SIGNAL_PORT, SIGNAL_VALUE_KIND, PortDirection::Input)],
            outputs: Vec::new(),
            configuration: Vec::new(),
            limits: limits(SIGNAL_VALUE_KIND, 16, 4, 64),
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
            limits: limits(GENERIC_VALUE_KIND, 16, 4, 64),
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
            limits: limits(GENERIC_VALUE_KIND, 16, 4, 64),
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
            limits: limits(GENERIC_VALUE_KIND, 16, 4, 64),
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
            outputs: vec![port(TEXT_PORT, TEXT_VALUE_KIND, PortDirection::Output)],
            configuration: vec![u64_field("template-id", 0, 0, u64::MAX)],
            limits: limits(TEXT_VALUE_KIND, 16, 4, 256),
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
            outputs: vec![port(TICK_PORT, TICK_VALUE_KIND, PortDirection::Output)],
            configuration: vec![
                u64_field("count", 16, 0, 4_096),
                u64_field("period-ms", 1_000, 0, u64::MAX),
            ],
            limits: limits(TICK_VALUE_KIND, 16, 4, 64),
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
            limits: limits(GENERIC_VALUE_KIND, 16, 4, 64),
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
    value_kind: &str,
    max_active_instances: u16,
    max_queue_items: u16,
    max_queue_bytes: u32,
) -> CapabilityLimits {
    CapabilityLimits {
        value_kind: kind_id(value_kind),
        max_active_instances,
        max_queue_items,
        max_queue_bytes,
    }
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

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::vec;

    use super::{
        find_contract, standard_contracts, standard_profile_catalog, FILTER_KIND, FORMAT_KIND,
        GENERIC_VALUE_KIND, LATEST_KIND, MAP_KIND, PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND,
        TEE_KIND, TEXT_VALUE_KIND, TICK_KIND,
    };
    use conduit_core::{
        kind_id, ArtifactId, CapabilityId, CapabilityOffer, ConnectionProvider, HostAdvertisement,
        HostId, HostProfileId, ImplementationId, OfferGeneration, PROTOCOL_VERSION,
    };
    use conduit_form::parse;
    use conduit_planner::{plan, PlacementChoice, PlacementChoices};

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
        assert_eq!(
            find_contract(&kind_id(MAP_KIND))
                .expect("map contract exists")
                .limits
                .value_kind,
            kind_id(GENERIC_VALUE_KIND)
        );
    }

    #[test]
    fn contracts_convert_to_form_catalog_without_runtime_kind_changes() {
        let catalog = standard_profile_catalog();
        let form = parse(
            "form 0\n\nstd_catalog {\n pulse: flow/pulse\n show: presentation/show\n pulse > show\n}\n",
            &catalog,
        )
        .expect("existing pulse/show form parses through standard catalog");
        assert_eq!(form.operations.len(), 2);
        assert_eq!(form.connections.len(), 1);

        let flow_form = parse(
            "form 0\n\nstd_flow {\n source: flow/map\n filtered: flow/filter\n latest: state/latest\n split: flow/tee\n formatted: text/format\n clock: time/tick\n source > filtered\n filtered > latest\n}\n",
            &catalog,
        )
        .expect("new standard flow form parses");
        assert_eq!(flow_form.operations.len(), 6);
        assert_eq!(flow_form.connections.len(), 2);
    }

    #[test]
    fn conformance_fixture_plans_standard_contracts_without_ui() {
        let catalog = standard_profile_catalog();
        let form = parse(
            "form 0\n\nstd_conformance {\n source: flow/map\n filter: flow/filter\n latest: state/latest\n split: flow/tee\n format: text/format\n clock: time/tick\n source > filter\n filter > latest\n}\n",
            &catalog,
        )
        .expect("standard conformance form parses");
        let host = standard_host_advertisement();
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
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
                    conduit_core::OperationId::from(operation),
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
            &[ConnectionProvider::Local],
        )
        .expect("standard conformance form plans");
        assert_eq!(plan.fragments.len(), 1);
        let fragment = &plan.fragments[0];
        assert_eq!(fragment.placements.len(), 6);
        assert_eq!(fragment.connections.len(), 2);
        assert!(fragment
            .placements
            .iter()
            .all(|placement| placement.implementation_id.as_str().starts_with("std/")));
    }

    #[test]
    fn platform_manifestation_truth_is_explicit() {
        let contracts = standard_contracts();
        let show = contracts
            .iter()
            .find(|contract| contract.kind_id.as_str() == SHOW_KIND)
            .expect("show contract exists");
        assert!(show.browser_manifestation_honest);
        assert!(show.pico_manifestation_honest);
        for contract in contracts
            .iter()
            .filter(|contract| contract.kind_id.as_str() != SHOW_KIND)
        {
            if contract.kind_id.as_str() != PULSE_KIND {
                assert!(!contract.browser_manifestation_honest);
                assert!(!contract.pico_manifestation_honest);
            }
        }
    }

    fn standard_host_advertisement() -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("std-catalog-host"),
            boot_id: conduit_core::BootId::from("std-catalog-boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("conduit.std/conformance"),
            capabilities: vec![
                offer("flow-pulse", PULSE_KIND, SIGNAL_VALUE_KIND, "std/pulse-v1"),
                offer(
                    "presentation-show",
                    SHOW_KIND,
                    SIGNAL_VALUE_KIND,
                    "std/show-v1",
                ),
                offer("flow-map", MAP_KIND, GENERIC_VALUE_KIND, "std/map-v1"),
                offer(
                    "flow-filter",
                    FILTER_KIND,
                    GENERIC_VALUE_KIND,
                    "std/filter-v1",
                ),
                offer("flow-tee", TEE_KIND, GENERIC_VALUE_KIND, "std/tee-v1"),
                offer(
                    "text-format",
                    FORMAT_KIND,
                    TEXT_VALUE_KIND,
                    "std/text-format-v1",
                ),
                offer(
                    "time-tick",
                    TICK_KIND,
                    super::TICK_VALUE_KIND,
                    "std/time-tick-v1",
                ),
                offer(
                    "state-latest",
                    LATEST_KIND,
                    GENERIC_VALUE_KIND,
                    "std/latest-v1",
                ),
            ],
        }
    }

    fn offer(
        capability: &str,
        kind: &str,
        value_kind: &str,
        implementation: &str,
    ) -> CapabilityOffer {
        CapabilityOffer {
            capability_id: CapabilityId::from(capability),
            kind_id: kind_id(kind),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(alloc::format!("conduit-std-catalog/{kind}").as_str()),
            limits: conduit_core::CapabilityLimits {
                value_kind: kind_id(value_kind),
                max_active_instances: 16,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        }
    }
}

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::{
    kind_id, port_id, resource_requirement, wait_host_operation_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationEntry, ConfigurationValue,
    ExecutionProfileId, HostOperationRequirement, ImplementationId, KindContractRevision, KindId,
    PortDescriptor, PortDirection, ResourceRequirement, TIMER_RESOURCE_CLASS,
};

pub const TICK_VALUE_KIND: &str = "value/tick@1";
pub const TICK_ENCODED_LEN: u32 = 8;
pub const TICK_CONTRACT_REVISION: &str = "conduit.std/time-tick@2";
pub const TICK_EXECUTION_PROFILE: &str = "conduit.std/time-tick-kernel-hosted@2";
pub const TICK_IMPLEMENTATION: &str = "std/kernel-time-tick@2";
pub const TICK_ARTIFACT: &str = "conduit-std-host/time-tick@2";
pub const TICK_CAPABILITY: &str = "time-tick-v2";
pub const MAX_TICK_COUNT: u64 = 4_096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickConfiguration {
    pub count: u64,
    pub period_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickContractError {
    MissingConfiguration(&'static str),
    InvalidConfiguration(&'static str),
    WrongEncodedLength(usize),
}

impl core::fmt::Display for TickContractError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingConfiguration(key) => write!(formatter, "missing configuration '{key}'"),
            Self::InvalidConfiguration(key) => write!(formatter, "invalid configuration '{key}'"),
            Self::WrongEncodedLength(length) => {
                write!(formatter, "wrong encoded tick length {length}")
            }
        }
    }
}

pub fn tick_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(super::TICK_KIND),
        plain_name: "Tick".to_string(),
        summary: "Emit a finite sequence of typed timer ticks.".to_string(),
        inputs: Vec::new(),
        outputs: tick_outputs(),
        configuration: vec![
            StandardConfigurationField {
                key: "count".to_string(),
                default_value: ConfigurationValue::U64(4),
                rule: StandardConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: MAX_TICK_COUNT,
                },
            },
            StandardConfigurationField {
                key: "period-ms".to_string(),
                default_value: ConfigurationValue::U64(1_000),
                rule: StandardConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: u64::MAX,
                },
            },
        ],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
        terminal_behavior: TerminalBehavior::CompletesAfterConfiguredCount,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "clock: time/tick".to_string(),
    }
}

pub fn tick_contract_revision() -> KindContractRevision {
    KindContractRevision::from(TICK_CONTRACT_REVISION)
}

pub fn tick_execution_profile() -> ExecutionProfileId {
    ExecutionProfileId::from(TICK_EXECUTION_PROFILE)
}

pub fn tick_outputs() -> Vec<PortDescriptor> {
    vec![PortDescriptor {
        port_id: port_id(super::TICK_PORT),
        value_kind: kind_id(TICK_VALUE_KIND),
        direction: PortDirection::Output,
        temporal: conduit_core::PortTemporal::Flow { closes: true },
    }]
}

pub fn tick_host_operation_requirements() -> Vec<HostOperationRequirement> {
    vec![wait_host_operation_requirement()]
}

pub fn tick_resource_requirements() -> Vec<ResourceRequirement> {
    vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)]
}

pub fn tick_capability_offer() -> CapabilityOffer {
    let contract = tick_contract();
    CapabilityOffer {
        startup_parameters: tick_face_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(TICK_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: tick_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: tick_execution_profile(),
            implementation_id: ImplementationId::from(TICK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TICK_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: tick_host_operation_requirements(),
        resource_requirements: tick_resource_requirements(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

pub fn tick_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
    vec![
        conduit_core::FaceStartupParameter {
            name: "count".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
        conduit_core::FaceStartupParameter {
            name: "period-ms".to_string(),
            value_type: "Count".to_string(),
            has_default: true,
        },
    ]
}

#[cfg(feature = "form-catalog")]
pub fn tick_profile_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

    let contract = tick_contract();
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: tick_contract_revision(),
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
                        StandardConfigurationRule::DurationMillis { minimum, maximum } => {
                            ConfigurationRule::DurationMillis { minimum, maximum }
                        }
                        StandardConfigurationRule::TextBytes { maximum } => {
                            ConfigurationRule::TextBytes { maximum }
                        }
                    },
                })
                .collect(),
        })
        .expect("the one-kind tick catalog has a unique semantic identity");
    catalog
}

#[cfg(feature = "form-catalog")]
pub fn install_tick_pipeline_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindSignature, StartupParameterSignature};
    startup.insert(KindSignature {
        kind: super::TICK_KIND.to_string(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "count".to_string(),
                value_type: "Count".to_string(),
                default: Some("4".to_string()),
            },
            StartupParameterSignature {
                name: "period-ms".to_string(),
                value_type: "Count".to_string(),
                default: Some("1000".to_string()),
            },
        ],
    })?;
    startup.insert(KindSignature {
        kind: super::TICK_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some("4".to_string()),
        }],
    })?;
    let tick = tick_profile_catalog();
    let presentation = super::tick_presentation_kind_definition();
    let tick_kind_id = conduit_core::kind_id(super::TICK_KIND);
    profile
        .insert(
            tick.get(&tick_kind_id)
                .expect("tick profile contains its exact kind")
                .clone(),
        )
        .map_err(|error| error.to_string())?;
    profile
        .insert(presentation)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn parse_tick_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TickConfiguration, TickContractError> {
    let mut count = None;
    let mut period_ms = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("period-ms", ConfigurationValue::U64(value)) => period_ms = Some(*value),
            ("count", _) => return Err(TickContractError::InvalidConfiguration("count")),
            ("period-ms", _) => {
                return Err(TickContractError::InvalidConfiguration("period-ms"));
            }
            _ => {}
        }
    }
    let count = count.ok_or(TickContractError::MissingConfiguration("count"))?;
    if count > MAX_TICK_COUNT {
        return Err(TickContractError::InvalidConfiguration("count"));
    }
    Ok(TickConfiguration {
        count,
        period_ms: period_ms.ok_or(TickContractError::MissingConfiguration("period-ms"))?,
    })
}

pub fn encode_tick(sequence: u64) -> [u8; TICK_ENCODED_LEN as usize] {
    sequence.to_le_bytes()
}

pub fn decode_tick(encoded: &[u8]) -> Result<u64, TickContractError> {
    let bytes: [u8; TICK_ENCODED_LEN as usize] = encoded
        .try_into()
        .map_err(|_| TickContractError::WrongEncodedLength(encoded.len()))?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn tick_value_kind() -> KindId {
    kind_id(TICK_VALUE_KIND)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_tick_contract_and_codec_are_exact() {
        let contract = tick_contract();
        assert_eq!(contract.kind_id.as_str(), super::super::TICK_KIND);
        assert_eq!(contract.outputs, tick_outputs());
        assert_eq!(contract.outputs[0].value_kind, tick_value_kind());
        assert_eq!(tick_contract_revision().as_str(), TICK_CONTRACT_REVISION);
        assert_eq!(decode_tick(&encode_tick(42)), Ok(42));
        assert!(matches!(
            decode_tick(&[0; 7]),
            Err(TickContractError::WrongEncodedLength(7))
        ));
    }

    #[test]
    fn zero_count_is_valid_and_over_maximum_is_rejected() {
        let parsed = parse_tick_configuration(&[
            ConfigurationEntry {
                key: "count".to_string(),
                value: ConfigurationValue::U64(0),
            },
            ConfigurationEntry {
                key: "period-ms".to_string(),
                value: ConfigurationValue::U64(7),
            },
        ])
        .expect("zero-count tick is terminal without effects");
        assert_eq!(parsed.count, 0);
        assert_eq!(parsed.period_ms, 7);

        assert_eq!(
            parse_tick_configuration(&[
                ConfigurationEntry {
                    key: "count".to_string(),
                    value: ConfigurationValue::U64(MAX_TICK_COUNT + 1),
                },
                ConfigurationEntry {
                    key: "period-ms".to_string(),
                    value: ConfigurationValue::U64(0),
                },
            ]),
            Err(TickContractError::InvalidConfiguration("count"))
        );
    }
}

use conduit_core::{
    kind_id, port_id, resource_requirement, wait_host_operation_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationEntry, ConfigurationValue,
    ExecutionProfileId, ImplementationId, KindContractRevision, PortDescriptor, PortDirection,
    TIMER_RESOURCE_CLASS,
};

pub(crate) use conduit_std_catalog::{text_presentation_offer as text_offer, MAX_TEXT_BYTES};
#[cfg(test)]
pub(crate) use conduit_std_catalog::{
    MAX_TEXT_VALUES, TEXT_PRESENTATION_CONTRACT_REVISION, TEXT_PRESENTATION_IMPLEMENTATION,
    TEXT_PRESENTATION_KIND, TEXT_PRESENTATION_VALUE_KIND,
};

pub(crate) const TICK_KIND: &str = "time/tick";
pub(crate) const TICK_VALUE_KIND: &str = "value/tick@1";
pub(super) const TICK_ENCODED_LEN: u32 = 8;
pub(crate) const TICK_CONTRACT_REVISION: &str = "conduit.std/time-tick@2";
pub(super) const TICK_EXECUTION_PROFILE: &str = "conduit.std/time-tick-kernel-hosted@2";
pub(super) const TICK_IMPLEMENTATION: &str = "std/kernel-time-tick@2";
pub(super) const TICK_ARTIFACT: &str = "conduit-std-host/time-tick@2";
const TICK_CAPABILITY: &str = "time-tick-v2";
const MAX_TICK_COUNT: u64 = 4_096;

pub(super) use conduit_std_catalog::{
    TIME_EVERY_ARTIFACT, TIME_EVERY_CONTRACT_REVISION, TIME_EVERY_COUNT,
    TIME_EVERY_EXECUTION_PROFILE, TIME_EVERY_IMPLEMENTATION, TIME_EVERY_KIND,
};

pub(super) struct TickConfiguration {
    pub(super) count: u64,
    pub(super) period_ms: u64,
}

pub(crate) fn every_offer() -> CapabilityOffer {
    conduit_std_catalog::time_every_offer()
}

pub(super) fn parse_every_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TickConfiguration, String> {
    if entries.len() != 1 {
        return Err("time/every requires exactly one planned configuration field".to_string());
    }
    let period_ms = entries
        .iter()
        .find_map(|entry| match (entry.key.as_str(), &entry.value) {
            ("freq", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or_else(|| "missing or invalid time/every configuration 'freq'".to_string())?;
    Ok(TickConfiguration {
        count: TIME_EVERY_COUNT,
        period_ms,
    })
}

pub(crate) fn tick_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: tick_face_startup_parameters(),
        shorthand: None,
        capability_id: CapabilityId::from(TICK_CAPABILITY),
        kind_id: kind_id(TICK_KIND),
        kind_contract_revision: KindContractRevision::from(TICK_CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(TICK_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(TICK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(TICK_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("tick"),
            value_kind: kind_id(TICK_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: conduit_core::PortTemporal::Flow { closes: true },
        }],
        host_operations: vec![wait_host_operation_requirement()],
        resource_requirements: vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

fn tick_face_startup_parameters() -> Vec<conduit_core::FaceStartupParameter> {
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

pub(super) fn parse_tick_configuration(
    entries: &[ConfigurationEntry],
) -> Result<TickConfiguration, String> {
    let mut count = None;
    let mut period_ms = None;
    for entry in entries {
        match (entry.key.as_str(), &entry.value) {
            ("count", ConfigurationValue::U64(value)) => count = Some(*value),
            ("period-ms", ConfigurationValue::U64(value)) => period_ms = Some(*value),
            ("count", _) => return Err("invalid tick configuration 'count'".to_string()),
            ("period-ms", _) => {
                return Err("invalid tick configuration 'period-ms'".to_string());
            }
            _ => {}
        }
    }
    let count = count.ok_or_else(|| "missing tick configuration 'count'".to_string())?;
    if count > MAX_TICK_COUNT {
        return Err("invalid tick configuration 'count'".to_string());
    }
    Ok(TickConfiguration {
        count,
        period_ms: period_ms.ok_or_else(|| "missing tick configuration 'period-ms'".to_string())?,
    })
}

pub(super) fn encode_tick(sequence: u64) -> [u8; TICK_ENCODED_LEN as usize] {
    sequence.to_le_bytes()
}

pub(super) fn decode_tick(encoded: &[u8]) -> Result<u64, String> {
    let bytes: [u8; TICK_ENCODED_LEN as usize] = encoded
        .try_into()
        .map_err(|_| format!("tick payload is {} bytes, expected 8", encoded.len()))?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
pub(super) fn test_tick_catalog() -> conduit_form::ProfileCatalog {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};

    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(TICK_KIND),
            kind_contract_revision: KindContractRevision::from(TICK_CONTRACT_REVISION),
            inputs: Vec::new(),
            outputs: tick_offer().outputs,
            configuration: vec![
                ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(4),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: MAX_TICK_COUNT,
                    },
                },
                ConfigurationField {
                    key: "period-ms".to_string(),
                    default_value: ConfigurationValue::U64(1_000),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: u64::MAX,
                    },
                },
            ],
        })
        .expect("the test catalog has one exact typed tick revision");
    catalog
}

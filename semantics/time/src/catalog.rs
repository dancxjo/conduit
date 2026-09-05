use alloc::string::{String, ToString};
use alloc::vec;
use conduit_core::{kind_id, ConfigurationValue, KindContractRevision};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};

use crate::{
    tick_outputs, MAX_TICK_COUNT, PHASE_SYNCHRONIZE_KIND, PHASE_SYNCHRONIZE_REVISION,
    PULSE_OBSERVATION_VALUE_KIND, PULSE_OBSERVE_KIND, PULSE_OBSERVE_REVISION,
    RHYTHM_STATE_VALUE_KIND, TICK_CONTRACT_REVISION, TICK_KIND, TICK_VALUE_KIND,
    TIME_EVERY_CONTRACT_REVISION, TIME_EVERY_KIND,
};
use conduit_core::{port_id, PortDescriptor, PortDirection, PortTemporal};

pub fn tick_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TICK_KIND),
        kind_contract_revision: KindContractRevision::from(TICK_CONTRACT_REVISION),
        inputs: alloc::vec::Vec::new(),
        outputs: tick_outputs(),
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
    }
}

pub fn time_every_kind_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TIME_EVERY_KIND),
        kind_contract_revision: KindContractRevision::from(TIME_EVERY_CONTRACT_REVISION),
        inputs: alloc::vec::Vec::new(),
        outputs: tick_outputs(),
        configuration: vec![ConfigurationField {
            key: "freq".to_string(),
            default_value: ConfigurationValue::U64(1_000),
            validation: ConfigurationRule::DurationMillis {
                minimum: 0,
                maximum: u64::MAX,
            },
        }],
    }
}

pub fn install_tick_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: TICK_KIND.to_string(),
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
    profile
        .insert(tick_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn install_time_every_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: TIME_EVERY_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "freq".to_string(),
            value_type: "Duration".to_string(),
            default: None,
        }],
    })?;
    profile
        .insert(time_every_kind_definition())
        .map_err(|error| error.to_string())
}

pub fn install_rhythm_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    insert_rhythm_kind(
        startup,
        profile,
        PULSE_OBSERVE_KIND,
        PULSE_OBSERVE_REVISION,
        vec![flow_port("tick", TICK_VALUE_KIND, PortDirection::Input)],
        vec![flow_port(
            "observation",
            PULSE_OBSERVATION_VALUE_KIND,
            PortDirection::Output,
        )],
    )?;
    insert_rhythm_kind(
        startup,
        profile,
        PHASE_SYNCHRONIZE_KIND,
        PHASE_SYNCHRONIZE_REVISION,
        vec![
            flow_port("local", RHYTHM_STATE_VALUE_KIND, PortDirection::Input),
            flow_port("peer", PULSE_OBSERVATION_VALUE_KIND, PortDirection::Input),
        ],
        vec![flow_port(
            "updated",
            RHYTHM_STATE_VALUE_KIND,
            PortDirection::Output,
        )],
    )
}

fn insert_rhythm_kind(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
    kind: &str,
    revision: &'static str,
    inputs: alloc::vec::Vec<PortDescriptor>,
    outputs: alloc::vec::Vec<PortDescriptor>,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: kind.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindDefinition {
            kind_id: kind_id(kind),
            kind_contract_revision: KindContractRevision::from(revision),
            inputs,
            outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn flow_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn definitions_preserve_exact_identities_and_bounds() {
        let tick = tick_kind_definition();
        assert_eq!(tick.kind_id.as_str(), TICK_KIND);
        assert_eq!(tick.outputs[0].value_kind.as_str(), crate::TICK_VALUE_KIND);
        assert_eq!(tick.kind_contract_revision.as_str(), TICK_CONTRACT_REVISION);
        let every = time_every_kind_definition();
        assert_eq!(every.kind_id.as_str(), TIME_EVERY_KIND);
        assert_eq!(
            every.kind_contract_revision.as_str(),
            TIME_EVERY_CONTRACT_REVISION
        );
    }
}

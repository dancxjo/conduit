//! Portable finite selection of the final normalized pattern in a closing Flow.

use alloc::{string::String, string::ToString, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const FINAL_NORMALIZED_PATTERN_KIND: &str = "sequence/final-normalized-pattern";
pub const FINAL_NORMALIZED_PATTERN_REVISION: &str = "conduit.sequence/final-normalized-pattern@1";
pub const DEFAULT_FINAL_PATTERN_VALUES: u64 = 4;
pub const MAXIMUM_FINAL_PATTERN_VALUES: u64 = 16;

pub fn final_normalized_pattern_definition() -> KindDefinition {
    let value_kind = crate::normalized_duration_sequence_type()
        .profile()
        .expect("reviewed normalized pattern type")
        .value_kind()
        .clone();
    KindDefinition {
        kind_id: kind_id(FINAL_NORMALIZED_PATTERN_KIND),
        kind_contract_revision: KindContractRevision::from(FINAL_NORMALIZED_PATTERN_REVISION),
        inputs: vec![PortDescriptor {
            port_id: port_id("patterns"),
            value_kind: value_kind.clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("pattern"),
            value_kind,
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        configuration: vec![ConfigurationField {
            key: "maximum-values".into(),
            default_value: ConfigurationValue::U64(DEFAULT_FINAL_PATTERN_VALUES),
            validation: ConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAXIMUM_FINAL_PATTERN_VALUES,
            },
        }],
    }
}

pub fn final_normalized_pattern_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 8,
        max_queue_items: MAXIMUM_FINAL_PATTERN_VALUES as u16,
        max_queue_bytes: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
            * MAXIMUM_FINAL_PATTERN_VALUES as u32,
    }
}

pub fn install_final_normalized_pattern_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert(KindSignature {
            kind: FINAL_NORMALIZED_PATTERN_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "maximum-values".into(),
                value_type: "Count".into(),
                default: Some(DEFAULT_FINAL_PATTERN_VALUES.to_string()),
            }],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(final_normalized_pattern_definition())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_closes_a_bounded_flow_into_one_exact_value() {
        let definition = final_normalized_pattern_definition();
        assert_eq!(
            definition.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(definition.outputs[0].temporal, PortTemporal::Value);
        assert_eq!(
            definition.inputs[0].value_kind,
            definition.outputs[0].value_kind
        );
        assert_eq!(definition.configuration.len(), 1);
    }
}

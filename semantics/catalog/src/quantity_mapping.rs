//! Portable mapping from a bounded scalar range to an exact unit-bearing quantity.

use alloc::{format, string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal, Quantity, QuantityUnit, Scalar, QUANTITY_INFO_ID,
    SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};

use crate::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};

pub const QUANTITY_MAP_KIND: &str = "math/map-quantity";
pub const QUANTITY_MAP_REVISION: &str = "conduit.std/math-map-quantity@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangePolicy {
    Refuse,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizationPolicy {
    Exact,
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantityMapping {
    pub source_minimum: Scalar,
    pub source_maximum: Scalar,
    pub target_minimum: i64,
    pub target_maximum: i64,
    pub target_granularity: i64,
    pub target_unit: QuantityUnit,
    pub range_policy: RangePolicy,
    pub quantization: QuantizationPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityMappingRefusal {
    InvalidRange,
    OutOfRange,
    Inexact,
    Overflow,
}

impl QuantityMapping {
    pub fn validate(self) -> Result<Self, QuantityMappingRefusal> {
        let target_span = i128::from(self.target_maximum) - i128::from(self.target_minimum);
        if self.source_minimum >= self.source_maximum
            || self.target_minimum > self.target_maximum
            || self.target_granularity <= 0
            || target_span.rem_euclid(i128::from(self.target_granularity)) != 0
        {
            return Err(QuantityMappingRefusal::InvalidRange);
        }
        Ok(self)
    }

    pub fn map(self, input: Scalar) -> Result<Quantity, QuantityMappingRefusal> {
        let mapping = self.validate()?;
        let input = if input < mapping.source_minimum || input > mapping.source_maximum {
            match mapping.range_policy {
                RangePolicy::Refuse => return Err(QuantityMappingRefusal::OutOfRange),
                RangePolicy::Clamp => input
                    .max(mapping.source_minimum)
                    .min(mapping.source_maximum),
            }
        } else {
            input
        };
        let source_span = i128::from(mapping.source_maximum.raw_microunits())
            - i128::from(mapping.source_minimum.raw_microunits());
        let source_offset = i128::from(input.raw_microunits())
            - i128::from(mapping.source_minimum.raw_microunits());
        let target_span = i128::from(mapping.target_maximum) - i128::from(mapping.target_minimum);
        let numerator = source_offset
            .checked_mul(target_span)
            .ok_or(QuantityMappingRefusal::Overflow)?;
        let step = i128::from(mapping.target_granularity);
        let denominator = source_span
            .checked_mul(step)
            .ok_or(QuantityMappingRefusal::Overflow)?;
        let lower_steps = numerator.div_euclid(denominator);
        let remainder = numerator.rem_euclid(denominator);
        let steps = match mapping.quantization {
            QuantizationPolicy::Exact if remainder != 0 => {
                return Err(QuantityMappingRefusal::Inexact)
            }
            QuantizationPolicy::Exact => lower_steps,
            QuantizationPolicy::Nearest if remainder * 2 >= denominator => lower_steps + 1,
            QuantizationPolicy::Nearest => lower_steps,
        };
        let mapped = i128::from(mapping.target_minimum)
            .checked_add(
                steps
                    .checked_mul(step)
                    .ok_or(QuantityMappingRefusal::Overflow)?,
            )
            .ok_or(QuantityMappingRefusal::Overflow)?
            .clamp(
                i128::from(mapping.target_minimum),
                i128::from(mapping.target_maximum),
            );
        Ok(Quantity::new(
            i64::try_from(mapped).map_err(|_| QuantityMappingRefusal::Overflow)?,
            mapping.target_unit,
        ))
    }
}

pub fn quantity_map_contract() -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(QUANTITY_MAP_KIND),
        plain_name: "Map scalar to quantity".into(),
        summary: "Map one bounded scalar into one exact unit-bearing quantity.".into(),
        inputs: vec![port("in", SCALAR_INFO_ID, PortDirection::Input)],
        outputs: vec![port("out", QUANTITY_INFO_ID, PortDirection::Output)],
        configuration: configuration_fields(),
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
        terminal_behavior: TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: "map: math/map-quantity(source-minimum = 0, source-maximum = 1000000, target-minimum = 20, target-maximum = 20000, target-granularity = 1, unit = \"Hz\", range-policy = \"clamp\", quantization = \"nearest\")".into(),
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_quantity_mapping_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    let contract = quantity_map_contract();
    startup.insert(KindSignature {
        kind: QUANTITY_MAP_KIND.into(),
        startup_parameters: contract
            .configuration
            .iter()
            .map(|field| StartupParameterSignature {
                name: field.key.clone(),
                value_type: match field.default_value {
                    ConfigurationValue::I64(_) => "Scalar",
                    ConfigurationValue::Text(_) => "Text",
                    _ => unreachable!(),
                }
                .into(),
                default: Some(match &field.default_value {
                    ConfigurationValue::I64(value) => value.to_string(),
                    ConfigurationValue::Text(value) => format!("\"{value}\""),
                    _ => unreachable!(),
                }),
            })
            .collect(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: KindContractRevision::from(QUANTITY_MAP_REVISION),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: contract
                .configuration
                .into_iter()
                .map(|field| ConfigurationField {
                    key: field.key,
                    default_value: field.default_value,
                    validation: match field.rule {
                        StandardConfigurationRule::I64Range { minimum, maximum } => {
                            ConfigurationRule::I64Range { minimum, maximum }
                        }
                        StandardConfigurationRule::TextOneOf { values } => {
                            ConfigurationRule::TextOneOf { values }
                        }
                        _ => unreachable!(),
                    },
                })
                .collect(),
        })
        .map_err(|error| error.to_string())
}

fn configuration_fields() -> Vec<StandardConfigurationField> {
    let number = |key: &str, value: i64| StandardConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::I64(value),
        rule: StandardConfigurationRule::I64Range {
            minimum: i64::MIN,
            maximum: i64::MAX,
        },
    };
    let choice = |key: &str, value: &str, values: &[&str]| StandardConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(value.into()),
        rule: StandardConfigurationRule::TextOneOf {
            values: values.iter().map(|value| (*value).into()).collect(),
        },
    };
    vec![
        number("source-minimum", 0),
        number("source-maximum", Scalar::SCALE),
        number("target-minimum", 0),
        number("target-maximum", 100),
        number("target-granularity", 1),
        choice(
            "unit",
            "Hz",
            &[
                "ns", "us", "ms", "s", "mHz", "Hz", "uV", "mV", "V", "um", "mm", "cm", "m", "udeg",
                "mdeg", "deg", "ppm", "permille", "%", "one", "B", "KiB", "MiB",
            ],
        ),
        choice("range-policy", "refuse", &["refuse", "clamp"]),
        choice("quantization", "exact", &["exact", "nearest"]),
    ]
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(policy: RangePolicy, quantization: QuantizationPolicy) -> QuantityMapping {
        QuantityMapping {
            source_minimum: Scalar::ZERO,
            source_maximum: Scalar::ONE,
            target_minimum: 20,
            target_maximum: 20_000,
            target_granularity: 10,
            target_unit: QuantityUnit::Hertz,
            range_policy: policy,
            quantization,
        }
    }

    #[test]
    fn boundaries_units_and_nearest_quantization_are_exact() {
        let value = mapping(RangePolicy::Clamp, QuantizationPolicy::Nearest);
        assert_eq!(
            value.map(Scalar::ZERO),
            Ok(Quantity::new(20, QuantityUnit::Hertz))
        );
        assert_eq!(
            value.map(Scalar::ONE),
            Ok(Quantity::new(20_000, QuantityUnit::Hertz))
        );
        assert_eq!(
            value.map(Scalar::from_raw_microunits(500_000)),
            Ok(Quantity::new(10_010, QuantityUnit::Hertz))
        );
        assert_eq!(
            value.map(Scalar::from_raw_microunits(2_000_000)),
            Ok(Quantity::new(20_000, QuantityUnit::Hertz))
        );
    }

    #[test]
    fn refusal_and_quantization_failures_remain_distinct() {
        assert_eq!(
            mapping(RangePolicy::Refuse, QuantizationPolicy::Nearest)
                .map(Scalar::from_raw_microunits(-1)),
            Err(QuantityMappingRefusal::OutOfRange)
        );
        assert_eq!(
            mapping(RangePolicy::Refuse, QuantizationPolicy::Exact)
                .map(Scalar::from_raw_microunits(1)),
            Err(QuantityMappingRefusal::Inexact)
        );
        let mut invalid = mapping(RangePolicy::Refuse, QuantizationPolicy::Exact);
        invalid.target_granularity = 7;
        assert_eq!(
            invalid.validate(),
            Err(QuantityMappingRefusal::InvalidRange)
        );
    }
}

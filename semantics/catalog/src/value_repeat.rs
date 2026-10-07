//! Explicit finite Value-to-Flow repetition; count is authored startup truth.
use alloc::vec;
use conduit_core::*;
pub const VALUE_REPEAT_KIND: &str = "value/repeat/finite";
pub const VALUE_REPEAT_REVISION: &str = "conduit.value/repeat-finite@1";
pub const VALUE_REPEAT_MAXIMUM_COUNT: u16 = 256;
pub fn value_repeat_semantic_contract(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    super::seeded_state_semantic_contract(value, schema)?;
    let input = PortDescriptor {
        port_id: port_id("value"),
        value_kind: value.value_kind.clone(),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("items"),
        value_kind: value.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Ok(Kind {
        kind_id: kind_id(VALUE_REPEAT_KIND),
        kind_contract_revision: KindIdentity::from(VALUE_REPEAT_REVISION),
        startup_parameters: vec![FrontStartupParameter {
            name: "count".into(),
            value_type: kind_id(COUNT_INFO_ID),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![input],
        outputs: vec![output],
        configuration: vec![KindConfigurationField {
            key: "count".into(),
            default_value: ConfigurationValue::U64(1),
            rule: KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: u64::from(VALUE_REPEAT_MAXIMUM_COUNT),
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("value")),
                    contract: value.clone(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("items")),
                    contract: value.clone(),
                },
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesAfterConfiguredCount),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: value
                .maximum_bytes
                .checked_mul(2)
                .ok_or("finite repeat storage overflows")?
                .max(1),
        },
    })
}
#[cfg(feature = "plot-catalog")]
pub fn install_value_repeat_kind(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    startup.insert(conduit_plot::KindSignature {
        kind: VALUE_REPEAT_KIND.into(),
        startup_parameters: vec![conduit_plot::StartupParameterSignature {
            name: "count".into(),
            value_type: "Count".into(),
            default: None,
        }],
    })?;
    profile
        .insert_kind(value_repeat_semantic_contract(value, schema).map_err(str::to_string)?)
        .map_err(|error| alloc::format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_fore_and_positive_count_bounds_are_retained() {
        let schema = StructuredInfoType::leaf(kind_id(SCALAR_INFO_ID)).unwrap();
        let value =
            CheckedValueContract::new(kind_id(SCALAR_INFO_ID), SCALAR_ENCODED_LEN as u32, vec![])
                .unwrap();
        let kind = value_repeat_semantic_contract(&value, &schema).unwrap();
        kind.validate().unwrap();
        assert_eq!(kind.inputs[0].temporal, PortTemporal::Value);
        assert_eq!(
            kind.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert!(!kind.startup_parameters[0].has_default);
        assert_eq!(
            kind.configuration[0].rule,
            KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: 256
            }
        );
        for count in [0, 257] {
            assert!(conduit_plot::validate_configuration_value(
                &kind.configuration[0],
                &ConfigurationValue::U64(count),
            )
            .is_err());
        }
        assert!(conduit_plot::validate_configuration_value(
            &kind.configuration[0],
            &ConfigurationValue::U64(2),
        )
        .is_ok());
    }
    #[test]
    fn mismatched_schema_refuses_before_preparation() {
        let schema = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
        let value =
            CheckedValueContract::new(kind_id(SCALAR_INFO_ID), SCALAR_ENCODED_LEN as u32, vec![])
                .unwrap();
        assert!(value_repeat_semantic_contract(&value, &schema).is_err());
    }
}

//! One absolute physical numerical allowance, separate from exact Quantity.
use super::*;
pub const NUMERICAL_NEAR_KIND: &str = "math/near";
pub const NUMERICAL_NEAR_REVISION: &str = "numeric/quantity-near@1";
pub const NUMERICAL_NEAR_MAX_BYTES: u32 = NEAR_QUANTITY_EVIDENCE_MAX_BYTES as u32;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearQuantityAlgorithm {
    ExactRational,
    Binary32,
}
pub fn near_quantity_type() -> StructuredInfoType {
    super::named("NearQuantity")
}
pub fn near_quantity_semantic_contract() -> Kind {
    Kind {
        kind_id: kind_id(NUMERICAL_NEAR_KIND),
        kind_contract_revision: NUMERICAL_NEAR_REVISION.into(),
        startup_parameters: vec![FrontStartupParameter {
            name: "limit".into(),
            value_type: kind_id(QUANTITY_INFO_ID),
            has_default: false,
        }],
        shorthand: Some((port_id("value"), port_id("result"))),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(QUANTITY_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: near_quantity_type().profile().unwrap().value_kind().clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "limit".into(),
            default_value: ConfigurationValue::Quantity(
                QuantityConfigurationValue::from_value(Quantity::new(0, Unit::One)).unwrap(),
            ),
            rule: KindConfigurationRule::Quantity,
        }],
        semantic_laws: vec![
            KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
                property_profile: quantity_realization_profile(),
                configuration_key: "limit".into(),
            }),
            KindSemanticLaw::Terminal(
                KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
            ),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("value")),
                    contract: CheckedValueContract::new(
                        kind_id(QUANTITY_INFO_ID),
                        QUANTITY_VALUE_MAX_ENCODED_BYTES as u32,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("result")),
                    contract: CheckedValueContract::new(
                        near_quantity_type().profile().unwrap().value_kind().clone(),
                        NUMERICAL_NEAR_MAX_BYTES,
                        vec![],
                    )
                    .unwrap(),
                },
            ]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 32,
            max_queue_items: 1,
            max_queue_bytes: NUMERICAL_NEAR_MAX_BYTES,
        },
    }
}
pub fn near_quantity_limit(
    configuration: &[ConfigurationEntry],
) -> Result<RationalQuantity, String> {
    let [entry] = configuration else {
        return Err("numeric/near-one-limit-required@1".into());
    };
    let ("limit", ConfigurationValue::Quantity(value)) = (entry.key.as_str(), &entry.value) else {
        return Err("numeric/near-physical-limit-required@1".into());
    };
    let value = value.value();
    if value.role() == QuantityRole::Point {
        return Err("numeric/near-limit-is-a-point@1".into());
    }
    let limit = RationalQuantity::from_quantity(value, &mut ExactNumericWork::new(100_000_000))
        .map_err(|why| format!("numeric/near-limit-invalid@1: {why:?}"))?;
    if limit.coordinate().is_negative() {
        return Err("numeric/near-negative-limit@1".into());
    }
    Ok(limit)
}
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let kind = near_quantity_semantic_contract();
    startup
        .insert(conduit_plot::KindSignature {
            kind: NUMERICAL_NEAR_KIND.into(),
            startup_parameters: vec![conduit_plot::StartupParameterSignature {
                name: "limit".into(),
                value_type: "Quantity".into(),
                default: None,
            }],
        })
        .map_err(|why| format!("near signature: {why:?}"))?;
    startup
        .insert_fore(NUMERICAL_NEAR_KIND, kind.checked_front())
        .map_err(|why| format!("near Fore: {why:?}"))?;
    profile
        .insert_kind(kind)
        .map_err(|why| format!("near Kind: {why:?}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn near_kind_and_source_fore_preserve_a_distinct_qualified_result() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_numerical_catalogs(&mut startup, &mut profile).unwrap();
        near_quantity_semantic_contract().validate().unwrap();
        assert_ne!(
            near_quantity_type().profile().unwrap().value_kind(),
            &kind_id(QUANTITY_INFO_ID)
        );
        let source="plot project (\n >> value: Quantity <= 1800B\n result: NearQuantity <= 16384B >>\n) {\n boundary: math/near(limit=0.0001m)\n value >> boundary.value\n boundary.result >> result\n}\n";
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let authored =
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "project", &profile)
                .unwrap();
        let gear = &authored.expanded.gears[0];
        assert_eq!(gear.kind_id.as_str(), NUMERICAL_NEAR_KIND);
        assert!(near_quantity_limit(&gear.configuration).is_ok());
    }
    #[test]
    fn negative_limits_and_affine_points_refuse_before_projection() {
        for (literal, name) in [("-1m", "negative-limit"), ("1°C", "limit-is-a-point")] {
            let config = [ConfigurationEntry {
                key: "limit".into(),
                value: ConfigurationValue::Quantity(
                    QuantityConfigurationValue::from_value(
                        Quantity::parse_plot_literal(literal).unwrap(),
                    )
                    .unwrap(),
                ),
            }];
            assert!(near_quantity_limit(&config).unwrap_err().contains(name));
        }
    }
}

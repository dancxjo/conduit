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
    #[test]
    fn absolute_fore_qualification_retains_physical_source_and_consuming_tolerance() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_numerical_catalogs(&mut startup, &mut profile).unwrap();
        let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n";
        let syntax = conduit_plot::parse_syntax_document(source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        let front = checked.plots[0].checked_front();
        assert_eq!(
            front.inputs()[0].value_kind,
            near_quantity_type().profile().unwrap().value_kind().clone()
        );
        assert_eq!(front.value_contracts().len(), 2);
        for entry in front.value_contracts() {
            let [ValueConstraint::NearQuantity {
                source_kind, limit, ..
            }] = entry.contract.constraints.as_slice()
            else {
                panic!("qualified contract")
            };
            assert_eq!(source_kind.as_str(), FREQUENCY_INFO_ID);
            assert_eq!(limit.unit(), Unit::resolve("Hz").unwrap());
        }
        assert_ne!(
            front.value_contracts()[0].contract.identity_bytes(),
            front.value_contracts()[1].contract.identity_bytes()
        );
        let expanded =
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "accept", &profile)
                .unwrap();
        assert_eq!(expanded.front, front);
    }
    #[test]
    fn qualified_fore_never_implicitly_becomes_exact_or_accepts_invalid_tolerance() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_numerical_catalogs(&mut startup, &mut profile).unwrap();
        for (input, output) in [
            ("Frequency near 0.01Hz", "Frequency"),
            ("Frequency", "Frequency near 0.01Hz"),
            ("Frequency near -0.01Hz", "Frequency near -0.01Hz"),
            ("Frequency near 0.01m", "Frequency near 0.01m"),
            ("Frequency near 0.01Hz near 0.1Hz", "NearQuantity"),
            ("Count near 0Hz", "NearQuantity"),
        ] {
            let source = format!("plot refuse (\n >> value: {input}\n result: {output} >>\n) {{\n value >> result\n}}\n");
            assert!(
                conduit_plot::check_syntax_document(
                    &conduit_plot::parse_syntax_document(&source),
                    &startup
                )
                .is_err(),
                "{input} -> {output}"
            );
        }
    }
    #[test]
    fn source_inlining_refuses_a_gear_that_would_drop_the_fore_predicate() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_numerical_catalogs(&mut startup, &mut profile).unwrap();
        let mut unqualified = near_quantity_semantic_contract();
        unqualified.kind_id = kind_id("test/unqualified-near-consumer");
        unqualified.kind_contract_revision = "test/unqualified-near-consumer@1".into();
        unqualified.startup_parameters.clear();
        unqualified.configuration.clear();
        unqualified.semantic_laws.clear();
        unqualified.inputs[0].value_kind =
            near_quantity_type().profile().unwrap().value_kind().clone();
        startup
            .insert(conduit_plot::KindSignature {
                kind: unqualified.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        startup
            .insert_fore(unqualified.kind_id.as_str(), unqualified.checked_front())
            .unwrap();
        profile.insert_kind(unqualified).unwrap();
        let source = "plot dropped (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) {\n consumer: test/unqualified-near-consumer\n value >> consumer.value\n consumer.result >> result\n}\n";
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(source),
            &startup,
        )
        .unwrap();
        let error =
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "dropped", &profile)
                .unwrap_err();
        assert_eq!(error.code, "CND-FRM-046");
        assert!(error.message.contains("retain its checked predicate"));
    }
    #[test]
    fn a_second_projection_cannot_discard_incoming_numerical_error() {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        install_numerical_catalogs(&mut startup, &mut profile).unwrap();
        let source = "plot again (\n >> value: Frequency near 0.01Hz\n result: NearQuantity <= 16384B >>\n) {\n second: math/near(limit=0.000000001Hz)\n value >> second.value\n second.result >> result\n}\n";
        let syntax = conduit_plot::parse_syntax_document(source);
        match conduit_plot::check_syntax_document(&syntax, &startup) {
            Err(error) => assert!(error.message.contains("Type") || error.message.contains("type")),
            Ok(checked) => {
                let error =
                    conduit_plot::expand_canonical_plot_for_authoring(&checked, "again", &profile)
                        .unwrap_err();
                assert_eq!(error.code, "CND-FRM-045");
            }
        }
    }
}

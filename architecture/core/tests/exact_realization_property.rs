use conduit_core::*;

fn property(name: &str, byte: u8) -> StructuredConfigurationValue {
    let leaf = StructuredInfoType::leaf(kind_id("value/u8")).unwrap();
    let ty = StructuredInfoType::nominal(kind_id(name), leaf.clone()).unwrap();
    let value = StructuredInfoValue::nominal(
        ty.clone(),
        StructuredInfoValue::leaf(leaf, vec![byte]).unwrap(),
    )
    .unwrap();
    StructuredConfigurationValue::new(
        ty.profile().unwrap().value_kind().clone(),
        value.canonical_bytes().unwrap(),
    )
    .unwrap()
}

#[test]
fn equality_requires_the_exact_profile_value_and_unique_bounded_properties() {
    let value = property("test/semantic-signature", 7);
    let requirement = ExactRealizationProperty {
        property_profile: value.profile().clone(),
        configuration_key: "signature".into(),
    };
    let request = ConfigurationValue::Structured(value.clone());
    requirement
        .check(Some(&request), std::slice::from_ref(&value))
        .unwrap();
    assert_eq!(
        requirement.check(None, &[]),
        Err(ExactRealizationRefusal::MissingConfiguration)
    );
    assert_eq!(
        requirement.check(Some(&request), &[]),
        Err(ExactRealizationRefusal::MissingProperty)
    );
    assert_eq!(
        requirement.check(Some(&request), &[property("test/semantic-signature", 8)]),
        Err(ExactRealizationRefusal::DifferentValue)
    );
    assert_eq!(
        requirement.check(
            Some(&ConfigurationValue::Structured(property(
                "test/other-signature",
                7
            ))),
            std::slice::from_ref(&value)
        ),
        Err(ExactRealizationRefusal::WrongConfigurationProfile)
    );
    assert_eq!(
        requirement.check(Some(&request), &[value.clone(), value]),
        Err(ExactRealizationRefusal::InvalidProperties)
    );
}

#[test]
fn exact_law_must_name_one_declared_matching_structured_configuration() {
    let value = property("test/semantic-signature", 7);
    let mut kind = Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "signature".into(),
            value_type: value.profile().clone(),
            has_default: false,
        }],
        shorthand: None,
        kind_id: kind_id("test/exact-property"),
        kind_contract_revision: "test/exact-property@1".into(),
        inputs: vec![],
        outputs: vec![],
        configuration: vec![KindConfigurationField {
            key: "signature".into(),
            default_value: ConfigurationValue::Structured(value.clone()),
            rule: KindConfigurationRule::Structured {
                profile: value.profile().clone(),
            },
        }],
        semantic_laws: vec![KindSemanticLaw::ExactRealizationProperty(
            ExactRealizationProperty {
                property_profile: value.profile().clone(),
                configuration_key: "signature".into(),
            },
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 262144,
        },
    };
    kind.validate().unwrap();
    kind.semantic_laws.push(kind.semantic_laws[0].clone());
    assert_eq!(
        kind.validate(),
        Err(KindValidationError::InvalidRealizationRequirement)
    );
    kind.semantic_laws.pop();
    let KindSemanticLaw::ExactRealizationProperty(requirement) = &mut kind.semantic_laws[0] else {
        unreachable!()
    };
    requirement.property_profile = property("test/other-signature", 7).profile().clone();
    assert_eq!(
        kind.validate(),
        Err(KindValidationError::InvalidRealizationRequirement)
    );
}

#[test]
fn native_quantity_equality_binds_coordinate_unit_and_role_without_a_new_request_type() {
    let q = QuantityConfigurationValue::from_value(Quantity::parse_plot_literal("1m").unwrap())
        .unwrap();
    let requirement = ExactRealizationProperty {
        property_profile: quantity_realization_profile(),
        configuration_key: "limit".into(),
    };
    let property = quantity_realization_property(&q).unwrap();
    let request = ConfigurationValue::Quantity(q);
    assert_eq!(
        requirement.check(Some(&request), std::slice::from_ref(&property)),
        Ok(())
    );
    let other =
        QuantityConfigurationValue::from_value(Quantity::parse_plot_literal("100cm").unwrap())
            .unwrap();
    // Equal physical magnitude does not erase the explicitly bound coordinates.
    assert_eq!(
        requirement.check(
            Some(&ConfigurationValue::Quantity(other)),
            std::slice::from_ref(&property)
        ),
        Err(ExactRealizationRefusal::DifferentValue)
    );
    let foreign = ExactRealizationProperty {
        property_profile: kind_id("foreign"),
        ..requirement
    };
    assert_eq!(
        foreign.check(Some(&request), &[property]),
        Err(ExactRealizationRefusal::WrongConfigurationProfile)
    );
}

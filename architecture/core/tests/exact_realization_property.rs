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
    requirement.check(Some(&request), &[value.clone()]).unwrap();
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
            &[value.clone()]
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

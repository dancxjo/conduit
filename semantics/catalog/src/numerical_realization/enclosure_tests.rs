use super::*;
fn expanded(width: &str) -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = format!("plot enclose (\n >> value: MathematicalValue <= 33059B\n result: MathematicalEnclosure <= 65536B >>\n) {{\n interval: math/enclose(request={{maximum-width:{width}}})\n value >> interval.value\n interval.result >> result\n}}\n");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(&checked, "enclose", &profile)
        .unwrap()
        .expanded
}
fn offer(property: StructuredConfigurationValue) -> CapabilityOffer {
    let mut offer = BackOfferBuilder::new(
        mathematical_enclose_semantic_contract(),
        Back {
            capability_id: "test/enclosure".into(),
            execution_profile_id: "test/interval".into(),
            implementation_id: "test/interval".into(),
            artifact_id: "test/artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.realization_properties = vec![property];
    offer
}
#[test]
fn checked_input_consumer_retains_typed_width_and_requires_the_same_offer_request() {
    let plot = expanded("0.00000000000000001");
    let gear = &plot.gears[0];
    assert_eq!(gear.kind_id.as_str(), MATHEMATICAL_ENCLOSE_KIND);
    let req = mathematical_enclose_requirements(&gear.configuration).unwrap();
    assert!(!req.maximum_width.is_negative());
    assert_ne!(req.maximum_width, ExactRational::ZERO);
    let ConfigurationValue::Structured(request) = &gear.configuration[0].value else {
        panic!()
    };
    assert!(gear.accepts_realization(&offer(request.clone())));
    let requirement = gear
        .semantic_contract
        .laws
        .iter()
        .find_map(|law| match law {
            KindSemanticLaw::ExactRealizationProperty(value) => Some(value),
            _ => None,
        })
        .unwrap();
    assert!(requirement
        .check(
            Some(&gear.configuration[0].value),
            &offer(request.clone()).realization_properties
        )
        .is_ok());
    let wrong = expanded("1");
    let ConfigurationValue::Structured(request) = &wrong.gears[0].configuration[0].value else {
        panic!()
    };
    assert!(requirement
        .check(
            Some(&gear.configuration[0].value),
            &offer(request.clone()).realization_properties
        )
        .is_err());
    let mut missing = offer(request.clone());
    missing.realization_properties.clear();
    assert!(requirement
        .check(
            Some(&gear.configuration[0].value),
            &missing.realization_properties
        )
        .is_err());
    let kind = mathematical_enclose_semantic_contract();
    assert_ne!(kind.inputs[0].value_kind, kind.outputs[0].value_kind);
}
#[test]
fn exact_width_is_permitted_but_negative_or_symbolic_requests_refuse_preparation() {
    let zero = expanded("0");
    assert_eq!(
        mathematical_enclose_requirements(&zero.gears[0].configuration)
            .unwrap()
            .maximum_width,
        ExactRational::ZERO
    );
    assert!(
        mathematical_enclose_requirements(&expanded("-1").gears[0].configuration)
            .unwrap_err()
            .contains("negative-width")
    );
    assert!(
        mathematical_enclose_requirements(&expanded("pi").gears[0].configuration)
            .unwrap_err()
            .contains("rational-width")
    );
    assert!(mathematical_enclose_requirements(&[]).is_err());
    let mut duplicated = zero.gears[0].configuration.clone();
    duplicated.push(duplicated[0].clone());
    assert!(mathematical_enclose_requirements(&duplicated).is_err());
}

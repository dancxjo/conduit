use super::*;
#[test]
fn evidence_envelope_holds_the_complete_shared_report_bound() {
    let ty = quantity_coordinate_evidence_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    let values = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(
                field.name(),
                StructuredInfoValue::leaf(field.value_type().clone(), vec![0; 65536]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let value = StructuredInfoValue::record(ty.clone(), values).unwrap();
    assert!(
        value.canonical_bytes().unwrap().len() <= QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES as usize
    );
}
fn expanded(unit: &str, declaration: &str) -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = format!("{declaration}\nplot convert (\n >> value: Quantity <= 33829B\n result: Quantity <= 33829B >>\n evidence: QuantityCoordinateEvidence <= 131500B >>\n) {{\n change: math/coordinates(request={{unit:{unit}}})\n value >> change.value\n change.result >> result\n change.evidence >> evidence\n}}\n");
    let syntax = conduit_plot::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    conduit_plot::expand_canonical_plot_for_authoring(&checked, "convert", &profile)
        .unwrap()
        .expanded
}
fn offer(property: StructuredConfigurationValue) -> CapabilityOffer {
    let mut result = BackOfferBuilder::new(
        quantity_coordinates_semantic_contract(),
        Back {
            capability_id: "test/coordinates".into(),
            execution_profile_id: "test/exact".into(),
            implementation_id: "test/coordinates".into(),
            artifact_id: "test/artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    result.realization_properties = vec![property];
    result
}
#[test]
fn authored_coordinate_consumer_preserves_ports_and_requires_exact_selected_unit() {
    let plot = expanded("cm", "");
    let gear = &plot.gears[0];
    assert_eq!(
        quantity_coordinate_target(&gear.configuration).unwrap(),
        Unit::Centimeter
    );
    let ConfigurationValue::Structured(request) = &gear.configuration[0].value else {
        panic!()
    };
    let offered = offer(request.clone());
    assert_eq!(gear.checked_front(), offered.checked_front());
    assert!(gear.accepts_realization(&offered));
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
            &offered.realization_properties
        )
        .is_ok());
    let wrong = expanded("m", "");
    let ConfigurationValue::Structured(other) = &wrong.gears[0].configuration[0].value else {
        panic!()
    };
    assert!(requirement
        .check(
            Some(&gear.configuration[0].value),
            &offer(other.clone()).realization_properties
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
    let kind = quantity_coordinates_semantic_contract();
    kind.validate().unwrap();
    assert_eq!(kind.inputs.len(), 1);
    assert_eq!(kind.outputs.len(), 2);
    assert_eq!(kind.inputs[0].value_kind, kind.outputs[1].value_kind);
    assert_ne!(kind.outputs[0].value_kind, kind.outputs[1].value_kind);
    assert!(quantity_coordinate_target(&[]).is_err());
    let mut duplicate = gear.configuration.clone();
    duplicate.push(duplicate[0].clone());
    assert!(quantity_coordinate_target(&duplicate).is_err());
}
#[test]
fn target_unit_uses_existing_authored_definition_binding() {
    let declaration = "unit span : Distance = { reference: m, scale: 2 }";
    let plot = expanded("span", declaration);
    let target = quantity_coordinate_target(&plot.gears[0].configuration).unwrap();
    assert!(target.matches_source_evidence("span"));
    let value = Quantity::new(3, Unit::Meter).convert(target).unwrap();
    assert_eq!(value.coefficient(), 15);
    assert_eq!(value.exponent(), -1);
    let old = target.encode();
    let changed = expanded("span", "unit span : Distance = { reference: m, scale: 3 }");
    assert_ne!(
        target,
        quantity_coordinate_target(&changed.gears[0].configuration).unwrap()
    );
    assert_eq!(Unit::decode(&old).unwrap(), target);
}

#[test]
fn coordinate_and_numeric_projections_form_distinct_authored_boundaries() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot chain (\n >> value: Quantity <= 33829B\n coordinate-evidence: QuantityCoordinateEvidence <= 131500B >>\n numeric-evidence: NearQuantity <= 131072B >>\n) {\n coordinates: math/coordinates(request={unit:cm})\n numeric: math/near(limit=0.001cm)\n value >> coordinates.value\n coordinates.result >> numeric.value\n coordinates.evidence >> coordinate-evidence\n numeric.result >> numeric-evidence\n}\n";
    let syntax = conduit_plot::parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(&checked, "chain", &profile)
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 2);
    let coordinate = expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == QUANTITY_COORDINATES_KIND)
        .unwrap();
    let numeric = expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == NUMERICAL_NEAR_KIND)
        .unwrap();
    assert_eq!(
        quantity_coordinate_target(&coordinate.configuration).unwrap(),
        Unit::Centimeter
    );
    assert_eq!(
        near_quantity_limit(&numeric.configuration).unwrap().unit(),
        Unit::Centimeter
    );
    assert_ne!(coordinate.semantic_contract, numeric.semantic_contract);
    assert_eq!(expanded.connections.len(), 1);
}

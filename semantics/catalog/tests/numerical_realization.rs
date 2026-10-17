use conduit_core::*;
use conduit_plot::{ProfileCatalog, StartupCatalog};
use conduit_semantic_catalog::*;

fn source() -> MathematicalValue {
    MathematicalValue::parse_literal("0.3", &mut ExactNumericWork::new(100_000_000)).unwrap()
}
fn prepared(algorithm: NumericalAlgorithm) -> PreparedNumericalRealization {
    PreparedNumericalRealization::prepare(
        &source(),
        algorithm,
        ExactRational::ONE,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap()
}
fn sink_kind() -> Kind {
    let result = numerical_result_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("proof/numeric-receipt"),
        kind_contract_revision: "proof/numeric-receipt@1".into(),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: result,
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 1,
            max_queue_bytes: NUMERICAL_RESULT_MAX_BYTES,
        },
    }
}
fn host(algorithm: NumericalAlgorithm, id: &str) -> HostAdvertisement {
    let mut offer = BackOfferBuilder::new(
        numerical_realize_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(id),
            execution_profile_id: ExecutionProfileId::from("proof/portable-numerics@1"),
            implementation_id: ImplementationId::from(id),
            artifact_id: ArtifactId::from("proof/portable-numerics@1"),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.realization_properties = vec![prepared(algorithm).property().unwrap()];
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(id),
        boot_id: BootId::from("numeric-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("proof/portable-numerics@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![
            offer,
            BackOfferBuilder::new(
                sink_kind(),
                Back {
                    capability_id: "proof/receipt".into(),
                    execution_profile_id: "proof/receipt".into(),
                    implementation_id: "proof/receipt".into(),
                    artifact_id: "proof/receipt".into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        ],
        planner_capabilities: vec![],
    }
}
fn plan() -> Plan {
    plan_for(NumericalAlgorithm::OutwardI64)
}
fn plan_for(approximation: NumericalAlgorithm) -> Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "proof/numeric-receipt".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore("proof/numeric-receipt", sink_kind().checked_front())
        .unwrap();
    profile.insert_kind(sink_kind()).unwrap();
    let plot = conduit_plot::parse_with_startup(
        "plot numerics {\n exact: math/realize(request={value:0.3,maximum-error:0})\n loose: math/realize(request={value:0.3,maximum-error:1})\n exact-receipt: proof/numeric-receipt\n loose-receipt: proof/numeric-receipt\n exact.result >> exact-receipt.value\n loose.result >> loose-receipt.value\n}\n",
        &startup, &profile).unwrap();
    plot.validate_identities().unwrap();
    let hosts = [
        host(approximation, "a-integer"),
        host(NumericalAlgorithm::ExactRational, "z-rational"),
    ];
    let mut choices = std::collections::BTreeMap::new();
    for gear in &plot.gears {
        let choice = conduit_planner::select_realization_with_policy(
            gear,
            &hosts,
            &conduit_planner::HardRealizationRequirements::default(),
            &conduit_planner::RealizationPolicy::default(),
        )
        .unwrap();
        choices.insert(gear.gear_id.clone(), choice);
    }
    for (receipt, producer) in [("exact-receipt", "exact"), ("loose-receipt", "loose")] {
        let source = plot
            .gears
            .iter()
            .find(|g| g.gear_id.as_str().ends_with(&format!("/{producer}")))
            .unwrap();
        let host_id = choices[&source.gear_id].host_id.clone();
        let sink = plot
            .gears
            .iter()
            .find(|g| g.gear_id.as_str().ends_with(&format!("/{receipt}")))
            .unwrap();
        choices.insert(
            sink.gear_id.clone(),
            conduit_planner::PlacementChoice {
                host_id,
                capability_id: "proof/receipt".into(),
            },
        );
    }
    conduit_planner::plan_with_options(
        &plot,
        &hosts,
        &conduit_planner::PlacementChoices { by_gear: choices },
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: NUMERICAL_RESULT_MAX_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}
#[test]
fn authored_precision_selects_real_numeric_kinds_and_seals_their_results() {
    let plan = plan();
    assert!(verify_plan(&plan));
    for gear in plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .filter(|g| g.kind_id.as_str() == NUMERICAL_REALIZE_KIND)
    {
        let p = PreparedNumericalRealization::from_property(
            &gear.realization_properties[0],
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
        let expected = if gear.gear_id.as_str().ends_with("exact") {
            NumericalAlgorithm::ExactRational
        } else {
            NumericalAlgorithm::OutwardI64
        };
        assert_eq!(p.algorithm(), expected, "{}", gear.gear_id.as_str());
        assert_eq!(p.source(), &source());
        let expected_error = if expected == NumericalAlgorithm::ExactRational {
            ExactRational::ZERO
        } else {
            MathematicalValue::parse_literal("0.7", &mut ExactNumericWork::new(100_000_000))
                .unwrap()
                .as_rational()
                .unwrap()
        };
        assert_eq!(p.maximum_error(), expected_error);
        if let NumericalTarget::OutwardI64(interval) = p.target() {
            assert_eq!(interval.lower().signed().unwrap(), 0);
            assert_eq!(interval.upper().signed().unwrap(), 1);
        }
        let result = numerical_result_value(&p).unwrap();
        let bytes = result.canonical_bytes().unwrap();
        assert_eq!(
            StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
            result
        );
        assert!(bytes.len() <= NUMERICAL_RESULT_MAX_BYTES as usize);
    }
}
#[cfg(feature = "kernel-step")]
#[test]
fn runtime_preparation_rechecks_selected_algorithm_ports_and_certificate() {
    let plan = plan();
    for gear in plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .filter(|g| g.kind_id.as_str() == NUMERICAL_REALIZE_KIND)
    {
        let p = PreparedNumericalRealization::from_property(
            &gear.realization_properties[0],
            &mut ExactNumericWork::new(100_000_000),
        )
        .unwrap();
        let back = NumericalRealizationBack::prepare(gear, p.algorithm()).unwrap();
        assert_eq!(
            back.result_bytes(),
            numerical_result_value(&p)
                .unwrap()
                .canonical_bytes()
                .unwrap()
        );
        let other = match p.algorithm() {
            NumericalAlgorithm::ExactRational => NumericalAlgorithm::OutwardI64,
            NumericalAlgorithm::OutwardI64 => NumericalAlgorithm::ExactRational,
            NumericalAlgorithm::Binary64 { .. } | NumericalAlgorithm::EnclosedBinary64 { .. } => {
                NumericalAlgorithm::ExactRational
            }
        };
        assert!(NumericalRealizationBack::prepare(gear, other).is_err());
        let mut changed = gear.clone();
        changed.outputs.clear();
        assert!(NumericalRealizationBack::prepare(&changed, p.algorithm()).is_err());
        let mut changed = gear.clone();
        changed.limits.max_queue_bytes = 1;
        assert!(NumericalRealizationBack::prepare(&changed, p.algorithm()).is_err());
        let mut changed = gear.clone();
        changed.configuration.push(changed.configuration[0].clone());
        assert!(NumericalRealizationBack::prepare(&changed, p.algorithm()).is_err());
        let mut changed = gear.clone();
        changed.realization_properties.clear();
        assert!(NumericalRealizationBack::prepare(&changed, p.algorithm()).is_err());
    }
}

#[cfg(feature = "kernel-step")]
#[path = "numerical_realization/execution.rs"]
mod execution;

#[test]
fn numerical_face_installer_requires_a_meaningful_explicit_label() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let signature = startup.signature(NUMERICAL_FACE_KIND).unwrap();
    assert_eq!(signature.startup_parameters[0].name, "label");
    assert!(signature.startup_parameters[0].default.is_none());
    let missing = format!("plot main {{\n result: {NUMERICAL_FACE_KIND}\n}}\n");
    let refusal = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&missing),
        &startup,
    )
    .unwrap_err();
    assert!(refusal.message.contains("label"), "{}", refusal.message);
    for argument in [
        "\"\"".to_owned(),
        "\"   \"".to_owned(),
        "42".to_owned(),
        "true".to_owned(),
        "\"Result\\nvalue\"".to_owned(),
        format!("{:?}", "x".repeat(129)),
    ] {
        let source = format!("plot main {{\n result: {NUMERICAL_FACE_KIND}({argument})\n}}\n");
        match conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        ) {
            Err(_) => {}
            Ok(checked) => assert!(
                conduit_plot::expand_canonical_plot(&checked, "main", &profile).is_err(),
                "accepted invalid label {argument}"
            ),
        }
    }
    for argument in [
        "\"Exact numerical result\"",
        "label=\"Exact numerical result\"",
    ] {
        let source = format!("plot main {{\n result: {NUMERICAL_FACE_KIND}({argument})\n}}\n");
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
        let expanded = conduit_plot::expand_canonical_plot(&checked, "main", &profile).unwrap();
        let gear = &expanded.gears[0];
        assert_eq!(
            face_data_label(&gear.configuration).unwrap(),
            "Exact numerical result"
        );
        assert_eq!(
            gear.configuration
                .iter()
                .find(|entry| entry.key == "label")
                .unwrap()
                .value,
            ConfigurationValue::Text("Exact numerical result".into())
        );
        assert_eq!(
            gear.inputs[0].value_kind,
            numerical_result_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        );
    }
}

#[cfg(feature = "kernel-step")]
#[test]
fn binary64_receipt_keeps_ieee_bits_rounding_status_and_error_kind() {
    let prepared = PreparedNumericalRealization::prepare(
        &source(),
        NumericalAlgorithm::Binary64 {
            rounding: Binary64Rounding::NearestTiesEven,
        },
        ExactRational::ONE,
        &mut ExactNumericWork::new(100_000_000),
    )
    .unwrap();
    let receipt = numerical_result_value(&prepared).unwrap();
    let StructuredInfoValueShape::Record(fields) = receipt.shape() else {
        panic!();
    };
    let representation = fields
        .iter()
        .find(|f| f.name() == "representation")
        .unwrap()
        .value();
    let StructuredInfoValueShape::Variant { tag, payload } = representation.shape() else {
        panic!();
    };
    assert_eq!(tag, "binary64");
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        panic!();
    };
    let target = fields
        .iter()
        .find(|f| f.name() == "target")
        .unwrap()
        .value();
    let StructuredInfoValueShape::Leaf(bytes) = target.shape() else {
        panic!();
    };
    assert_eq!(IeeeF64::decode(bytes).unwrap().bits(), 0x3fd3333333333333);
    let rounding = fields
        .iter()
        .find(|f| f.name() == "rounding")
        .unwrap()
        .value();
    assert!(
        matches!(rounding.shape(),StructuredInfoValueShape::Variant {tag,..} if tag=="nearest_ties_even")
    );
    let inexact = fields
        .iter()
        .find(|f| f.name() == "any-inexact")
        .unwrap()
        .value();
    assert!(matches!(inexact.shape(),StructuredInfoValueShape::Leaf(bytes) if bytes == [1]));
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&receipt.canonical_bytes().unwrap()).unwrap(),
        receipt
    );
}

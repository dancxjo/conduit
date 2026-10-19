//! Actual installed profile with checked Source, sealed bounds and shared kernel.
use super::*;
use crate::host_execution::scalar_tape_fixture::{NoTimer, Outputs};
use crate::ExternalForeInput;
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;
const BRIDGE: &str =
    "plot bridge (\n >> value: Angle\n result: Angle >>\n) = (math/angle-radians(.))\n";
const SINE: &str =
    "plot sine (\n >> value: Angle\n result: MathematicalValue >>\n) = (math/sin(.))\n";
const CHAIN:&str="plot bridge (\n >> value: Angle\n result: Angle >>\n) = (math/angle-radians(.))\nplot carry (\n >> value: Angle\n result: Angle >>\n) = (.)\nplot sine (\n >> value: Angle\n result: MathematicalValue >>\n) = (math/sin(.))\nplot main (\n >> value: Angle\n result: MathematicalValue >>\n) {\n converted: bridge\n carried: carry\n consumed: sine\n value >> converted.value\n converted.result >> carried.value\n carried.result >> consumed.value\n consumed.result >> result\n}\n";
fn fixture(source: &str, name: &str) -> (crate::StdHost, Plan) {
    fixture_with_selection(source, name, false)
}
fn fixture_with_selection(source: &str, name: &str, automatic: bool) -> (crate::StdHost, Plan) {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
    let mut advertisement = crate::StdHost::new().advertisement().clone();
    for gear in authored.expanded.gears.iter().filter(|_| !automatic) {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program");
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer = conduit_std_offers::pure_mathematical_angle_std_offer(
            &program,
            gear.inputs[0].temporal,
        )
        .unwrap();
        assert!(gear.accepts_realization(&offer));
        advertisement.capabilities.push(offer);
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let mut host = crate::StdHost::from_advertisement(advertisement).unwrap();
    if automatic {
        host.prepare_numerical_realizations(&authored.expanded)
            .unwrap();
        host.prepare_expression_realizations(&authored.expanded)
            .unwrap();
        let first = host.advertisement().clone();
        host.prepare_expression_realizations(&authored.expanded)
            .unwrap();
        assert_eq!(host.advertisement(), &first);
    }
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: conduit_semantic_catalog::NUMERICAL_NEAR_MAX_BYTES,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::NUMERICAL_NEAR_MAX_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    (host, plan)
}
fn play(mut host: crate::StdHost, plan: &Plan, input: Vec<u8>) -> Vec<u8> {
    let mut outputs = Outputs(Vec::with_capacity(1));
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: input,
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    let kernel = report.kernel.as_ref().unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(outputs.0.len(), 1);
    println!(
        "MATHEMATICAL_ANGLE_KERNEL plan={} source={} post_start_allocations={}",
        plan.plan_id.as_str(),
        plan.source_document_id.as_str(),
        kernel.post_play_start_allocations
    );
    outputs.0.remove(0)
}
#[test]
fn installed_conversion_and_sine_preserve_the_actual_transported_physical_frame() {
    let (host, plan) = fixture(BRIDGE, "bridge");
    let native = Quantity::parse_plot_literal("90°").unwrap();
    let bytes = play(host, &plan, native.encode().to_vec());
    let mut work = ExactNumericWork::new(200_000_000);
    let view = MathematicalQuantityView::decode(&bytes, &mut work).unwrap();
    assert_eq!(view.unit(), Unit::Radian);
    assert_eq!(view.role(), QuantityRole::Linear);
    assert_eq!(view.family(), Unit::Turn.family());
    assert_eq!(
        view.coordinate()
            .rational_pi_coefficient(&mut work)
            .unwrap(),
        Some(ExactRational::ratio(1, 2, &mut work).unwrap())
    );
    assert_eq!(
        plan.fragments[0].placements[0]
            .execution_profile_id
            .as_str(),
        conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE
    );
    let (host, consumer) = fixture(SINE, "sine");
    let result = play(host, &consumer, bytes);
    assert_eq!(
        MathematicalValueView::decode(&result, &mut work)
            .unwrap()
            .as_rational(&mut work)
            .unwrap(),
        Some(ExactRational::ONE)
    );
}
#[test]
fn one_authored_plan_runs_the_exact_angle_chain_through_selected_installed_backs() {
    for spelling in ["0.25turn", "90°"] {
        let (host, plan) = fixture(CHAIN, "main");
        assert_eq!(plan.fragments[0].placements.len(), 3);
        assert!(plan.fragments[0]
            .placements
            .iter()
            .all(|p| p.execution_profile_id.as_str()
                == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE));
        let result = play(
            host,
            &plan,
            Quantity::parse_plot_literal(spelling)
                .unwrap()
                .encode()
                .to_vec(),
        );
        let mut work = ExactNumericWork::new(200_000_000);
        assert_eq!(
            MathematicalValueView::decode(&result, &mut work)
                .unwrap()
                .as_rational(&mut work)
                .unwrap(),
            Some(ExactRational::ONE)
        );
    }
}
#[test]
fn installed_factory_recomputes_wire_work_storage_artifact_and_host_call_bounds() {
    let (_, plan) = fixture(BRIDGE, "bridge");
    let original = &plan.fragments[0].placements[0];
    assert!(PureExpressionHost::from_placement(original).is_ok());
    for index in 0..original.realization_properties.len() {
        let mut missing = original.clone();
        missing.realization_properties.remove(index);
        assert!(PureExpressionHost::from_placement(&missing).is_err());
        let mut changed = original.clone();
        let p = &changed.realization_properties[index];
        let mut bytes = p.canonical_value().to_vec();
        *bytes.last_mut().unwrap() ^= 1;
        changed.realization_properties[index] =
            StructuredConfigurationValue::new(p.profile().clone(), bytes).unwrap();
        assert!(PureExpressionHost::from_placement(&changed).is_err());
    }
    let index = original
        .realization_properties
        .iter()
        .position(|p| {
            let value = StructuredInfoValue::from_canonical_bytes(p.canonical_value()).unwrap();
            matches!(value.value_type().shape(),StructuredInfoTypeShape::Record { schema,.. }
            if schema.as_str()==conduit_std_offers::MATHEMATICAL_ANGLE_BOUNDS_SCHEMA)
        })
        .unwrap();
    for name in [
        "maximum-nodes",
        "maximum-depth",
        "retained-prepared-payload-ceiling-bytes",
        "maximum-numerical-work",
        "maximum-input-bytes",
        "output-capacity-bytes",
    ] {
        let mut changed = original.clone();
        let property = &changed.realization_properties[index];
        let value = StructuredInfoValue::from_canonical_bytes(property.canonical_value()).unwrap();
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("bounds record");
        };
        let mut fields = fields.to_vec();
        let position = fields.iter().position(|f| f.name() == name).unwrap();
        let field = &fields[position];
        let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
            panic!("u64");
        };
        let n = u64::from_le_bytes(bytes.try_into().unwrap()) + 1;
        fields[position] = StructuredFieldValue::new(
            name,
            StructuredInfoValue::leaf(field.value().value_type().clone(), n.to_le_bytes().to_vec())
                .unwrap(),
        )
        .unwrap();
        let value = StructuredInfoValue::record(value.value_type().clone(), fields).unwrap();
        changed.realization_properties[index] = StructuredConfigurationValue::new(
            property.profile().clone(),
            value.canonical_bytes().unwrap(),
        )
        .unwrap();
        assert!(
            PureExpressionHost::from_placement(&changed).is_err(),
            "{name}"
        );
    }
    let mut artifact = original.clone();
    artifact.artifact_id = "foreign/angle@1".into();
    assert!(PureExpressionHost::from_placement(&artifact).is_err());
    let mut limits = original.clone();
    limits.limits.max_queue_bytes = limits.limits.max_queue_bytes.saturating_sub(1);
    assert!(PureExpressionHost::from_placement(&limits).is_err());
    let mut call = original.clone();
    call.host_calls[0].maximum_input_bytes += 1;
    assert!(PureExpressionHost::from_placement(&call).is_err());
    let mut profile = original.clone();
    profile.execution_profile_id = conduit_std_offers::PURE_EXPRESSION_STD_PROFILE.into();
    profile.artifact_id = conduit_std_offers::PURE_EXPRESSION_STD_ARTIFACT.into();
    assert!(PureExpressionHost::from_placement(&profile).is_err());
}

#[test]
fn ordinary_source_preparation_selects_compatible_angle_profiles_across_the_chain() {
    let (host, plan) = fixture_with_selection(CHAIN, "main", true);
    assert_eq!(plan.fragments[0].placements.len(), 3);
    assert!(plan.fragments[0]
        .placements
        .iter()
        .all(|p| p.execution_profile_id.as_str()
            == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE));
    let result = play(
        host,
        &plan,
        Quantity::parse_plot_literal("90°")
            .unwrap()
            .encode()
            .to_vec(),
    );
    let mut work = ExactNumericWork::new(200_000_000);
    assert_eq!(
        MathematicalValueView::decode(&result, &mut work)
            .unwrap()
            .as_rational(&mut work)
            .unwrap(),
        Some(ExactRational::ONE)
    );
}

#[test]
fn authored_dimensioned_chain_reaches_selected_binary32_consumer_and_independent_reader() {
    let source = "plot bridge (\n >> value: Angle\n result: Angle >>\n) = (math/angle-radians(.))\nplot carry (\n >> value: Angle\n result: Angle >>\n) = (.)\nplot expose (\n >> value: Angle\n result: Quantity >>\n) = (quantity/value(.))\nplot project (\n >> value: Angle\n result: NearQuantity <= 131072B >>\n) {\n converted: bridge\n carried: carry\n exposed: expose\n boundary: math/near(limit=0.000001rad)\n value >> converted.value\n converted.result >> carried.value\n carried.result >> exposed.value\n exposed.result >> boundary.value\n boundary.result >> result\n}\n";
    let source = format!("plot halve (\n >> value: Angle\n result: Angle >>\n) = (. / 2)\nplot double (\n >> value: Angle\n result: Angle >>\n) = (. * 2)\n{}", source
        .replace(" converted: bridge", " halved: halve\n doubled: double\n converted: bridge")
        .replace(" value >> converted.value", " value >> halved.value\n halved.result >> doubled.value\n doubled.result >> converted.value"));
    for spelling in ["90°", "0.25turn"] {
        let (host, plan) = fixture_with_selection(&source, "project", true);
        assert_eq!(plan.fragments[0].placements.len(), 6);
        assert_eq!(
            plan.fragments[0]
                .placements
                .iter()
                .filter(|p| p.execution_profile_id.as_str()
                    == "std/quantity-near-enclosed-midpoint-binary32@1")
                .count(),
            1
        );
        let result = play(
            host,
            &plan,
            Quantity::parse_plot_literal(spelling)
                .unwrap()
                .encode()
                .to_vec(),
        );
        let requested = Quantity::parse_plot_literal("0.000001rad").unwrap();
        let mut work = ExactNumericWork::new(900_000_000);
        let mut verifier = PreparedMathematicalNearQuantityVerifier::new(
            &conduit_semantic_catalog::near_quantity_type(),
            crate::numerical_host::near_step::mathematical_near_requirements(),
            Binary32Rounding::NearestTiesEven,
            RationalQuantity::from_quantity(requested, &mut work).unwrap(),
            900_000_000,
        )
        .unwrap();
        let guard = crate::allocation_probe::begin();
        let value = verifier
            .verify(&result, ANGLE_INFO_ID, None, &mut work)
            .unwrap();
        assert_eq!(value.source().unit(), Unit::Radian);
        assert_eq!(value.source().role(), QuantityRole::Linear);
        assert_eq!(
            value
                .source()
                .coordinate()
                .rational_pi_coefficient(&mut work)
                .unwrap(),
            Some(ExactRational::ratio(1, 2, &mut work).unwrap())
        );
        assert_ne!(value.maximum_error(), ExactRational::ZERO);
        assert_eq!(guard.finish(), 0);
    }
}

#[test]
fn installed_exact_angle_arithmetic_feeds_the_symbolic_conversion_without_growth() {
    let transforms = "plot halve (\n >> value: Angle\n result: Angle >>\n) = (. / 2)\nplot double (\n >> value: Angle\n result: Angle >>\n) = (. * 2)\n";
    let source = format!("{transforms}{}", CHAIN
        .replace(" converted: bridge", " halved: halve\n doubled: double\n converted: bridge")
        .replace(" value >> converted.value", " value >> halved.value\n halved.result >> doubled.value\n doubled.result >> converted.value"));
    let (host, plan) = fixture_with_selection(&source, "main", true);
    assert_eq!(plan.fragments[0].placements.len(), 5);
    assert_eq!(
        plan.fragments[0]
            .placements
            .iter()
            .filter(|p| p.execution_profile_id.as_str()
                == conduit_std_offers::PURE_EXACT_QUANTITY_STD_PROFILE)
            .count(),
        2
    );
    let result = play(
        host,
        &plan,
        Quantity::parse_plot_literal("90°")
            .unwrap()
            .encode()
            .to_vec(),
    );
    assert_eq!(
        MathematicalValueView::decode(&result, &mut ExactNumericWork::new(200_000_000))
            .unwrap()
            .as_rational(&mut ExactNumericWork::new(200_000_000))
            .unwrap(),
        Some(ExactRational::ONE)
    );
}

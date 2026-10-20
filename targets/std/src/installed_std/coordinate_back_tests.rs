//! Actual Source/Plan/kernel conversion-to-Near execution, with separate evidence.
use super::*;
use crate::host_execution::scalar_tape_fixture::{NoTimer, Outputs};
use crate::{ExternalForeInput, StdHost};
use conduit_core::projection::*;
use conduit_plot::*;
use std::collections::BTreeMap;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(300_000_000)
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a [u8] {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!()
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value()
        .shape()
    else {
        panic!()
    };
    bytes
}
fn prepared() -> (StdHost, Plan) {
    prepared_for("cm", "0.01cm")
}
fn prepared_for(target: &str, limit: &str) -> (StdHost, Plan) {
    let source = "plot chain (\n >> value: Quantity <= 33829B\n coordinate-evidence: QuantityCoordinateEvidence <= 131500B >>\n numeric-evidence: NearQuantity <= 131072B >>\n) {\n coordinates: math/coordinates(request={unit:cm})\n numeric: math/near(limit=0.01cm)\n value >> coordinates.value\n coordinates.result >> numeric.value\n coordinates.evidence >> coordinate-evidence\n numeric.result >> numeric-evidence\n}\n";
    let source = source
        .replace("unit:cm", &format!("unit:{target}"))
        .replace("limit=0.01cm", &format!("limit={limit}"));
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "chain", &profile).unwrap();
    let abnormal = authored
        .abnormal_export
        .as_ref()
        .expect("physical refusal remains a separate Plot obligation");
    assert_eq!(abnormal.value_kind.as_str(), TERMINAL_INFO_ID);
    assert_eq!(abnormal.gear_id.as_str(), "chain/coordinates");
    assert_eq!(abnormal.gear_port_id.as_str(), "evidence");

    let mut host = StdHost::new();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "coordinate-evidence"),
        (PortDirection::Output, "numeric-evidence"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: match port {
                    "coordinate-evidence" => QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES,
                    "numeric-evidence" => NUMERICAL_NEAR_MAX_BYTES,
                    _ => MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
                },
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments[0].placements.len(), 2);
    assert_eq!(plan.source_document_id, syntax.source_document_id());
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|p| p.implementation_id.as_str() == IMPLEMENTATION));
    (host, plan)
}

#[test]
fn installed_exact_coordinates_feed_numeric_projection_with_independently_replayable_reports() {
    let (mut host, plan) = prepared();
    let mut sources = vec![Quantity::new(1, Unit::Meter).encode().to_vec()];
    for denominator in [1, 3] {
        let rational = RationalQuantity::new(
            ExactRational::ratio(1, denominator, &mut work()).unwrap(),
            Unit::Meter,
            QuantityRole::Linear,
        )
        .unwrap();
        let mut encoded = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
        let n = encode_rational_quantity_value(rational, &mut encoded).unwrap();
        encoded.truncate(n);
        sources.push(encoded);
    }
    let mut first_target = None;
    for (index, source) in sources.into_iter().enumerate() {
        let mut output = Outputs(Vec::with_capacity(2));
        let run = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: source.clone(),
                }],
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(output.0.len(), 2);
        let records = output
            .0
            .iter()
            .map(|b| StructuredInfoValue::from_canonical_bytes(b).unwrap())
            .collect::<Vec<_>>();
        let coordinate = records
            .iter()
            .find(|v| v.value_type() == &quantity_coordinate_evidence_type())
            .unwrap();
        let numeric = records
            .iter()
            .find(|v| v.value_type() == &near_quantity_type())
            .unwrap();
        let target = field(numeric, "source-wire");
        let view = QuantityValueProfile::Mathematical
            .decode(QUANTITY_INFO_ID, target, &mut work())
            .unwrap();
        assert_eq!(view.unit(), Unit::Centimeter);
        if index < 2 {
            let exact = view.rational(&mut work()).unwrap();
            assert_eq!(exact.coordinate(), ExactRational::from_i128(100));
            if let Some(first) = first_target {
                assert_eq!(exact, first);
            } else {
                first_target = Some(exact);
            }
        }

        let head = field(coordinate, "projection-evidence-head");
        let tail = field(coordinate, "projection-evidence-tail");
        assert_eq!(head.len(), (head.len() + tail.len()).min(65536));
        let mut frame = head.to_vec();
        frame.extend_from_slice(tail);
        let inspected = inspect_projection_evidence(&frame).unwrap();
        assert_eq!(&inspected.source_bytes[4..4 + source.len()], source);
        let ids = QuantityCoordinateContentIds::new(&source, target, Unit::Centimeter);
        let mut verifier =
            PreparedQuantityCoordinateConversion::new(500000, COORDINATE_WORK).unwrap();
        verifier
            .verify_evidence(
                &source,
                target,
                QuantityCoordinateConversionPolicy {
                    target_unit: Unit::Centimeter,
                },
                COORDINATE_WORK,
                ids.basis(),
                &frame,
                &mut vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES],
                &mut work(),
            )
            .unwrap();
        let consumer = PreparedNearQuantityVerifier::new(&near_quantity_type()).unwrap();
        let numeric_bytes = numeric.canonical_bytes().unwrap();
        consumer
            .verify(
                &numeric_bytes,
                QUANTITY_INFO_ID,
                RationalQuantity::from_quantity(
                    Quantity::parse_plot_literal("0.01cm").unwrap(),
                    &mut work(),
                )
                .unwrap(),
                Some(semantic_digest(QUANTITY_INFO_ID, target)),
                &mut ExactNumericWork::new(900_000_000),
            )
            .unwrap();
        let kernel = run.kernel.as_ref().unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
    }
}

#[test]
fn coordinate_installation_refuses_missing_wire_or_resource_proof_and_stale_identity() {
    let (_, plan) = prepared();
    let gear = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == QUANTITY_COORDINATES_KIND)
        .unwrap();
    assert!(admitted(gear).is_ok());
    let mut missing_wire = gear.clone();
    missing_wire
        .realization_properties
        .retain(|p| p.profile() != &QuantityWireContract::property_kind());
    assert!(admitted(&missing_wire).is_err());
    let mut stale = gear.clone();
    stale.artifact_id = "test/stale-artifact".into();
    assert!(admitted(&stale).is_err());
    let mut dropped = gear.clone();
    dropped.semantic_contract.laws.clear();
    assert!(admitted(&dropped).is_err());
    let mut missing_bounds = gear.clone();
    missing_bounds
        .realization_properties
        .retain(|p| p.profile() == &QuantityWireContract::property_kind());
    assert!(admitted(&missing_bounds).is_err());
}

#[test]
fn direct_evidence_fore_does_not_turn_physical_refusal_into_normal_completion() {
    let (mut host, plan) = prepared();
    let mut output = Outputs(Vec::new());
    let why = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: Quantity::new(1, Unit::Kelvin).encode().to_vec(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap_err();
    assert!(why.contains("SemanticAbnormal"), "{why}");
    assert!(!why.contains("BackFailed"), "{why}");
    assert!(output.0.is_empty());
}

#[path = "coordinate_back_tests/physical.rs"]
mod physical;

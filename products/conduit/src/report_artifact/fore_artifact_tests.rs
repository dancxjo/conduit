//! Real external-Fore execution through ordinary artifact export and inspection.
use conduit_core::*;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("Near has no timer effect")
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        assert_eq!(output.track, ConnectionTrack::Payload);
        self.0.push(output.bytes);
        Ok(())
    }
}

fn prepared(relay: bool, coordinate_request: Option<(&str, &str)>) -> (StdHost, Plan) {
    let coordinates = coordinate_request.is_some();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let prefix = if relay {
        "plot relay (\n >> value: Quantity <= 33829B\n result: Quantity <= 33829B >>\n) = .\n\n"
    } else {
        ""
    };
    let input = if relay {
        " relay: relay\n value >> relay.value\n relay.result >> boundary.value"
    } else {
        " value >> boundary.value"
    };
    let source = format!("{prefix}plot project (\n >> value: Quantity <= 33829B\n result: NearQuantity <= 131072B >>\n) {{\n boundary: math/near(limit=0.0001m)\n{input}\n boundary.result >> result\n}}\n");
    let source = if let Some((unit, limit)) = coordinate_request {
        "plot project (\n >> value: Quantity <= 33829B\n coordinate-evidence: QuantityCoordinateEvidence <= 131500B >>\n result: NearQuantity <= 131072B >>\n) {\n coordinates: math/coordinates(request={unit:cm})\n boundary: math/near(limit=0.01cm)\n value >> coordinates.value\n coordinates.result >> boundary.value\n coordinates.evidence >> coordinate-evidence\n boundary.result >> result\n}\n".replace("unit:cm", &format!("unit:{unit}")).replace("limit=0.01cm", &format!("limit={limit}"))
    } else {
        source
    };
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "project", &profile).unwrap();
    let mut host = StdHost::new();
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let hosts = [host.advertisement().clone()];
    let mut placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let near = authored
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::NUMERICAL_NEAR_KIND)
        .unwrap();
    placements
        .by_gear
        .get_mut(&near.gear_id)
        .unwrap()
        .capability_id = hosts[0]
        .capabilities
        .iter()
        .find(|o| {
            o.implementation.execution_profile_id.as_str()
                == "std/quantity-near-enclosed-midpoint-binary32@3"
        })
        .unwrap()
        .capability_id
        .clone();
    let mut boundary_ports = vec![
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ];
    if coordinates {
        boundary_ports.push((PortDirection::Output, "coordinate-evidence"));
    }
    let boundaries = boundary_ports
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
                    byte_capacity: if port == "value" {
                        33829
                    } else if port == "coordinate-evidence" {
                        131500
                    } else {
                        conduit_semantic_catalog::NUMERICAL_NEAR_MAX_BYTES
                    },
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
            connection_byte_capacity: 33829,
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

#[test]
fn actual_output_fore_bytes_export_and_inspect_without_a_core_cord() {
    let third = RationalQuantity::new(
        ExactRational::ratio(1, 3, &mut ExactNumericWork::new(100_000_000)).unwrap(),
        Unit::resolve("m").unwrap(),
        QuantityRole::Linear,
    )
    .unwrap();
    let mut rational = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let length = encode_rational_quantity_value(third, &mut rational).unwrap();
    rational.truncate(length);
    for input in [
        Quantity::parse_plot_literal("1m")
            .unwrap()
            .encode()
            .to_vec(),
        rational,
    ] {
        export_and_inspect(input, false, None);
    }
    export_and_inspect(
        Quantity::parse_plot_literal("1m")
            .unwrap()
            .encode()
            .to_vec(),
        true,
        None,
    );
}

#[test]
fn actual_coordinate_reports_export_and_replay_against_the_plan_request() {
    let mut work = ExactNumericWork::new(200_000_000);
    let pi = MathematicalValue::parse_literal("pi", &mut work).unwrap();
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = pi.encode(&mut coordinate).unwrap();
    let pi = MathematicalValueView::decode(&coordinate[..n], &mut work).unwrap();
    let mut pi_radians = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &pi,
        Unit::Radian,
        QuantityRole::Linear,
        &mut pi_radians,
        &mut work,
    )
    .unwrap();
    pi_radians.truncate(n);
    for (input, unit, limit) in [
        (
            Quantity::parse_plot_literal("1m")
                .unwrap()
                .encode()
                .to_vec(),
            "cm",
            "0.01cm",
        ),
        (
            Quantity::parse_plot_literal("1turn")
                .unwrap()
                .encode()
                .to_vec(),
            "rad",
            "0.01rad",
        ),
        (pi_radians, "turn", "0turn"),
        (
            Quantity::parse_plot_literal("1rad")
                .unwrap()
                .encode()
                .to_vec(),
            "turn",
            "0.001turn",
        ),
    ] {
        export_and_inspect(input, false, Some((unit, limit)));
    }
}

fn export_and_inspect(input: Vec<u8>, relay: bool, coordinate_request: Option<(&str, &str)>) {
    let input_proof = input.clone();
    let (mut host, plan) = prepared(relay, coordinate_request);
    let coordinates = coordinate_request.is_some();
    let advertisement = host.advertisement().clone();
    let mut outputs = Outputs::default();
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
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    let active_play = kernel.active_play.clone();
    let sign_identities = report
        .observations
        .iter()
        .map(|sign| {
            let retained = kernel.identity.sign_identity(&sign.sign_id).unwrap();
            let identity = bind_sign(
                &sign.host_id,
                &sign.boot_id,
                sign.active_play_id.as_ref(),
                retained.sequence,
            );
            assert_eq!(identity.sign_id, sign.sign_id);
            identity
        })
        .collect::<Vec<_>>();
    let routed_values = kernel
        .content_evidence
        .retained_values
        .iter()
        .cloned()
        .map(|value| crate::product_execution::RuntimeRoutedValue {
            active_play_id: active_play.active_play_id.clone(),
            value,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        routed_values.len(),
        if coordinates {
            3
        } else if relay {
            2
        } else {
            1
        }
    );
    assert_eq!(outputs.0.len(), if coordinates { 2 } else { 1 });
    assert!(routed_values
        .iter()
        .any(|value| value.value.bytes == outputs.0[0]));
    if coordinates {
        let records = outputs
            .0
            .iter()
            .map(|bytes| StructuredInfoValue::from_canonical_bytes(bytes).unwrap())
            .collect::<Vec<_>>();
        let coordinate = records
            .iter()
            .find(|record| {
                record.value_type()
                    == &conduit_semantic_catalog::quantity_coordinate_evidence_type()
            })
            .unwrap();
        let bytes = |record: &StructuredInfoValue, name: &str| {
            let StructuredInfoValueShape::Record(fields) = record.shape() else {
                panic!("record")
            };
            let StructuredInfoValueShape::Leaf(bytes) = fields
                .iter()
                .find(|field| field.name() == name)
                .unwrap()
                .value()
                .shape()
            else {
                panic!("bytes")
            };
            bytes.to_vec()
        };
        let mut evidence = bytes(coordinate, "projection-evidence-head");
        evidence.extend_from_slice(&bytes(coordinate, "projection-evidence-tail"));
        let view = conduit_core::projection::inspect_projection_evidence(&evidence).unwrap();
        let source_len = u32::from_le_bytes(view.source_bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(&view.source_bytes[4..4 + source_len], input_proof);
        let target_frame = view.target_bytes.unwrap();
        let target_len = u32::from_le_bytes(target_frame[..4].try_into().unwrap()) as usize;
        let target = &target_frame[4..4 + target_len];
        assert_ne!(target, input_proof);
        assert!(routed_values
            .iter()
            .any(|value| value.value.bytes == target));
        let numeric = records
            .iter()
            .find(|record| record.value_type() == &conduit_semantic_catalog::near_quantity_type())
            .unwrap();
        assert_eq!(bytes(numeric, "source-wire"), target);
    }
    let snapshot = crate::report_artifact::snapshot_from_execution(
        vec![advertisement],
        vec![],
        vec![plan],
        report.observations,
    );
    let directory = std::env::temp_dir().join(format!(
        "conduit-fore-artifacts-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    crate::report_artifact::write_execution_artifacts(
        &directory,
        &snapshot,
        std::slice::from_ref(&active_play),
        &sign_identities,
        &routed_values,
    )
    .unwrap();
    let mut missing_context = snapshot.clone();
    missing_context
        .observations
        .retain(|sign| !matches!(sign.kind, ObservationKind::ForeOutputContent { .. }));
    missing_context.retention.retained_items = missing_context.observations.len() as u32;
    missing_context.retention.dropped_items += 1;
    let retained_identities = sign_identities
        .iter()
        .filter(|identity| {
            missing_context
                .observations
                .iter()
                .any(|sign| sign.sign_id == identity.sign_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let refused_directory = directory.with_extension("missing-context");
    let refusal = crate::report_artifact::write_execution_artifacts(
        &refused_directory,
        &missing_context,
        &[active_play],
        &retained_identities,
        &routed_values,
    )
    .unwrap_err();
    assert!(
        refusal.contains("lacks a retained exact Cord or output-Fore content Sign"),
        "{refusal}"
    );
    assert!(!refused_directory.exists());
    let mut values = 0;
    let mut fore_values = 0;
    let mut inspections = Vec::new();
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(artifact) =
            serde_json::from_slice::<conduit_observatory::RoutedValueArtifact>(&bytes)
        else {
            continue;
        };
        values += 1;
        let inspection = crate::inspection::inspect(&path).unwrap();
        if matches!(
            artifact.sign.sign.kind,
            ObservationKind::RoutedContent { .. }
        ) {
            assert!((relay || coordinates) && artifact.sign.sign.connection_id.is_some());
            if relay {
                assert_eq!(artifact.bytes, input_proof);
            }
            inspections.push((path.with_extension("inspection.txt"), inspection));
            continue;
        }
        fore_values += 1;
        assert!(artifact.sign.sign.connection_id.is_none());
        assert!(matches!(
            artifact.sign.sign.kind,
            ObservationKind::ForeOutputContent { .. }
        ));
        assert!(outputs.0.contains(&artifact.bytes));
        assert!(
            inspection.contains("Projection report claims"),
            "{inspection}"
        );
        assert!(inspection.contains("Fore"), "{inspection}");
        if coordinate_request == Some(("turn", "0.001turn"))
            && inspection.contains("Numerical certificate independently replayed")
        {
            assert!(
                inspection.contains("target binary32 0x3e22f983"),
                "{inspection}"
            );
            assert!(
                !inspection.contains("maximum absolute error 0 turn\n"),
                "{inspection}"
            );
        }
        if coordinate_request == Some(("turn", "0turn"))
            && inspection.contains("Numerical certificate independently replayed")
        {
            assert!(
                inspection.contains("target binary32 0x3f000000"),
                "{inspection}"
            );
            assert!(
                inspection.contains("maximum absolute error 0 turn"),
                "{inspection}"
            );
            assert!(
                inspection.contains("requested allowance 0 turn"),
                "{inspection}"
            );
        }

        assert!(
            inspection.contains("Numerical certificate independently replayed")
                || (coordinates
                    && inspection.contains("Coordinate certificate independently replayed")),
            "{inspection}"
        );
        assert!(
            inspection.contains("original Plan request matched"),
            "{inspection}"
        );
        inspections.push((path.with_extension("inspection.txt"), inspection));
    }
    assert_eq!(values, routed_values.len());
    assert_eq!(fore_values, if coordinates { 2 } else { 1 });
    for (path, inspection) in inspections {
        std::fs::write(path, inspection).unwrap();
    }
    std::fs::write(directory.join("actual-input-wire.bin"), input_proof).unwrap();
    // Retain actual exported files for this execution's proof custody.
    println!("FORE_ARTIFACT_DIRECTORY={}", directory.display());
}

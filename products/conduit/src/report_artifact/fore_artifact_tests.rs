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

fn prepared(relay: bool) -> (StdHost, Plan) {
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
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
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
                byte_capacity: if port == "value" {
                    33829
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
        export_and_inspect(input, false);
    }
    export_and_inspect(
        Quantity::parse_plot_literal("1m")
            .unwrap()
            .encode()
            .to_vec(),
        true,
    );
}

fn export_and_inspect(input: Vec<u8>, relay: bool) {
    let input_proof = input.clone();
    let (mut host, plan) = prepared(relay);
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
    assert_eq!(routed_values.len(), if relay { 2 } else { 1 });
    assert_eq!(outputs.0.len(), 1);
    assert!(routed_values
        .iter()
        .any(|value| value.value.bytes == outputs.0[0]));
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
            assert!(relay && artifact.sign.sign.connection_id.is_some());
            assert_eq!(artifact.bytes, input_proof);
            inspections.push((path.with_extension("inspection.txt"), inspection));
            continue;
        }
        fore_values += 1;
        assert!(artifact.sign.sign.connection_id.is_none());
        assert!(matches!(
            artifact.sign.sign.kind,
            ObservationKind::ForeOutputContent { .. }
        ));
        assert_eq!(artifact.bytes, outputs.0[0]);
        assert!(
            inspection.contains("Projection report claims"),
            "{inspection}"
        );
        assert!(inspection.contains("Fore"), "{inspection}");
        assert!(
            inspection.contains("Numerical certificate independently replayed"),
            "{inspection}"
        );
        assert!(
            inspection.contains("original Plan request matched"),
            "{inspection}"
        );
        inspections.push((path.with_extension("inspection.txt"), inspection));
    }
    assert_eq!(values, routed_values.len());
    assert_eq!(fore_values, 1);
    for (path, inspection) in inspections {
        std::fs::write(path, inspection).unwrap();
    }
    std::fs::write(directory.join("actual-input-wire.bin"), input_proof).unwrap();
    // Retain actual exported files for this execution's proof custody.
    println!("FORE_ARTIFACT_DIRECTORY={}", directory.display());
}

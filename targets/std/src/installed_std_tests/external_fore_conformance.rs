use super::{host, installed_std, BTreeMap, BaseImplementationId, RecordingTimer};
use crate::{ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter};
use conduit_core::{ConnectionTrack, PortDirection};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

#[derive(Default)]
struct Collector(Vec<ExternalForeDelivery>);

impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output);
        Ok(())
    }
}

#[test]
fn ordinary_open_plot_runs_through_sealed_fore_and_observed_terminal() {
    let catalog = installed_std::test_catalog();
    let startup = catalog.startup_catalog().unwrap();
    let source = r#"plot uppercase (
 >> text: Text
 upper: Text >>
) {
 operation: text/upper
 text >> operation.source
operation.text >> upper
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, "uppercase", &catalog).unwrap();
    let mut host = host("external-fore-host");
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = [
        (
            ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: conduit_core::port_id("text"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 64,
            },
        ),
        (
            ForeBoundaryKey {
                direction: PortDirection::Output,
                front_port_id: conduit_core::port_id("upper"),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 64,
            },
        ),
    ]
    .into_iter()
    .collect();
    let empty = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let fragment = plan.fragments[0].clone();
    assert_eq!(fragment.fore_ports.len(), 2);
    let mut collector = Collector::default();
    let report = host
        .run_external_plot_to(
            fragment.clone(),
            &[ExternalForeInput {
                front_port_id: conduit_core::port_id("text"),
                track: ConnectionTrack::Payload,
                bytes: b"Hello, Fore".to_vec(),
            }],
            &mut collector,
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    assert_eq!(collector.0.len(), 1);
    assert_eq!(collector.0[0].front_port_id, conduit_core::port_id("upper"));
    assert_eq!(collector.0[0].bytes, b"HELLO, FORE");
    assert_eq!(collector.0, report.external_fore_deliveries);
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.fore_endpoints.len(), 2);
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::RemoteInputAdmitted));
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered));
    let refusal = host
        .run_external_plot_sequence_to(
            fragment,
            &[
                ExternalForeInput {
                    front_port_id: conduit_core::port_id("text"),
                    track: ConnectionTrack::Payload,
                    bytes: b"one".to_vec(),
                },
                ExternalForeInput {
                    front_port_id: conduit_core::port_id("text"),
                    track: ConnectionTrack::Payload,
                    bytes: b"two".to_vec(),
                },
            ],
            &mut Collector::default(),
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap_err();
    assert!(refusal.contains("one exact input Flow"));
}

#[test]
fn one_external_fore_payload_is_delivered_to_every_sealed_internal_branch() {
    let catalog = installed_std::test_catalog();
    let startup = catalog.startup_catalog().unwrap();
    let source = r#"plot twin-uppercase (
 >> text: Text
 first: Text >>
 second: Text >>
) {
 first-operation: text/upper
 second-operation: text/upper
 text >> first-operation.source
 text >> second-operation.source
 first-operation.text >> first
 second-operation.text >> second
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "twin-uppercase", &catalog).unwrap();
    assert_eq!(
        authoring
            .input_bindings
            .iter()
            .filter(|binding| binding.front_port_id == conduit_core::port_id("text"))
            .count(),
        2
    );
    let mut host = host("external-fore-fanout-host");
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = ["text", "first", "second"]
        .into_iter()
        .map(|port| {
            (
                ForeBoundaryKey {
                    direction: if port == "text" {
                        PortDirection::Input
                    } else {
                        PortDirection::Output
                    },
                    front_port_id: conduit_core::port_id(port),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 64,
                },
            )
        })
        .collect();
    let empty = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let fragment = plan.fragments[0].clone();
    assert_eq!(fragment.fore_ports.len(), 4);
    let mut collector = Collector::default();
    host.run_external_plot_to(
        fragment,
        &[ExternalForeInput {
            front_port_id: conduit_core::port_id("text"),
            track: ConnectionTrack::Payload,
            bytes: b"one payload".to_vec(),
        }],
        &mut collector,
        &mut Vec::new(),
        &mut RecordingTimer { waits: Vec::new() },
    )
    .unwrap();
    collector
        .0
        .sort_by(|left, right| left.front_port_id.cmp(&right.front_port_id));
    assert_eq!(collector.0.len(), 2);
    assert_eq!(collector.0[0].front_port_id, conduit_core::port_id("first"));
    assert_eq!(collector.0[0].bytes, b"ONE PAYLOAD");
    assert_eq!(
        collector.0[1].front_port_id,
        conduit_core::port_id("second")
    );
    assert_eq!(collector.0[1].bytes, b"ONE PAYLOAD");
}

#[test]
fn finite_sequence_uses_one_sealed_flow_and_delivers_in_order_under_capacity_one() {
    let mut catalog = conduit_plot::ProfileCatalog::new();
    let mut startup = conduit_plot::StartupCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut catalog).unwrap();
    conduit_tongues::install_speech_commit_catalog(&mut startup, &mut catalog).unwrap();
    startup
        .insert_value_kind_alias(
            "SpeakableText",
            conduit_core::kind_id(conduit_tongues::SPEAKABLE_TEXT_VALUE_KIND),
        )
        .unwrap();
    let source = "plot stream (\n >> text: Text...| <= 1024B\n segments: SpeakableText...| >>\n) {\n commit: speech/commit-generated-text\n text >> commit.generated\n commit.segments >> segments\n}.\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, "stream", &catalog).unwrap();
    let host_with_commit = || {
        let mut host = super::host("external-fore-sequence-host");
        host.advertisement
            .capabilities
            .push(conduit_std_offers::generated_speech_commit_offer());
        host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement).unwrap();
        host
    };
    let mut host = host_with_commit();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = ["text", "segments"]
        .into_iter()
        .map(|name| {
            (
                ForeBoundaryKey {
                    direction: if name == "text" {
                        PortDirection::Input
                    } else {
                        PortDirection::Output
                    },
                    front_port_id: conduit_core::port_id(name),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 2048,
                },
            )
        })
        .collect();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 2048,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    let fragment = plan.fragments[0].clone();
    let values = ["First. ", "Second. ", "Third."]
        .into_iter()
        .map(|text| ExternalForeInput {
            front_port_id: conduit_core::port_id("text"),
            track: ConnectionTrack::Payload,
            bytes: text.as_bytes().to_vec(),
        })
        .collect::<Vec<_>>();
    let mut collector = Collector::default();
    let report = host
        .run_external_plot_sequence_to(
            fragment.clone(),
            &values,
            &mut collector,
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    assert_eq!(collector.0.len(), 3);
    assert_eq!(
        collector
            .0
            .iter()
            .map(|value| value.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    let spoken = collector
        .0
        .iter()
        .map(|value| conduit_tongues::decode_speakable_segment(&value.bytes).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        spoken
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>(),
        vec!["First. ", "Second. ", "Third."]
    );
    assert_eq!(collector.0, report.external_fore_deliveries);
    assert!(matches!(
        report.observations.last().map(|event| &event.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.fore_endpoints.len(), 2);
    assert_eq!(
        kernel
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::RemoteInputAdmitted)
            .count(),
        3
    );

    let mut cancelled_host = host_with_commit();
    let control = crate::RunControl::default();
    let stop_id = crate::RunControlRequestId::new("stop-before-fore-feed").unwrap();
    control.request_stop(stop_id.clone()).unwrap();
    let mut cancelled_outputs = Collector::default();
    let cancelled = cancelled_host
        .run_external_plot_sequence_controlled_to(
            fragment.clone(),
            &values,
            &mut cancelled_outputs,
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
            &control,
        )
        .unwrap();
    assert!(cancelled_outputs.0.is_empty());
    assert_eq!(cancelled.control_receipts[0].request_id, stop_id);
    assert!(matches!(
        cancelled.observations.last().map(|event| &event.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Cancelled { .. }
        })
    ));

    struct StopAfterFirst {
        control: crate::RunControl,
        delivered: Vec<ExternalForeDelivery>,
    }
    impl ExternalForeOutputAdapter for StopAfterFirst {
        fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
            self.delivered.push(output);
            if self.delivered.len() == 1 {
                self.control
                    .request_stop(crate::RunControlRequestId::new("stop-after-first").unwrap())
                    .map_err(|error| format!("stop after first delivery: {error:?}"))?;
            }
            Ok(())
        }
    }
    let control = crate::RunControl::default();
    let mut partial_outputs = StopAfterFirst {
        control: control.clone(),
        delivered: Vec::new(),
    };
    let partial = host_with_commit()
        .run_external_plot_sequence_controlled_to(
            fragment,
            &values,
            &mut partial_outputs,
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
            &control,
        )
        .unwrap();
    assert_eq!(partial_outputs.delivered.len(), 1);
    assert_eq!(partial.control_receipts.len(), 1);
    assert!(matches!(
        partial.observations.last().map(|event| &event.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Cancelled { .. }
        })
    ));

    let mut wrong_port = values.clone();
    wrong_port[1].front_port_id = conduit_core::port_id("invented");
    assert!(host_with_commit()
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &wrong_port,
            &mut Collector::default(),
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap_err()
        .contains("sealed input Flow"));
    let oversized = vec![values[0].clone(); 65];
    assert!(host_with_commit()
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &oversized,
            &mut Collector::default(),
            &mut Vec::new(),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap_err()
        .contains("one to 64"));
}

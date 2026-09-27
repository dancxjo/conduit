use super::{host, installed_std, BTreeMap, BaseImplementationId, RecordingTimer};
use crate::{ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter};
use conduit_core::{ConnectionTrack, PortDirection};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
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
fn ordinary_open_form_runs_through_sealed_fore_and_observed_terminal() {
    let catalog = installed_std::test_catalog();
    let startup = catalog.startup_catalog().unwrap();
    let source = r#"form uppercase (
 >> text: Text
 upper: Text >>
) {
 operation: text/upper
 text >> operation.text
operation.text >> upper
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring = expand_canonical_form_for_authoring(&checked, "uppercase", &catalog).unwrap();
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
        .run_external_form_to(
            fragment,
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
}

#[test]
fn one_external_fore_payload_is_delivered_to_every_sealed_internal_branch() {
    let catalog = installed_std::test_catalog();
    let startup = catalog.startup_catalog().unwrap();
    let source = r#"form twin-uppercase (
 >> text: Text
 first: Text >>
 second: Text >>
) {
 first-operation: text/upper
 second-operation: text/upper
 text >> first-operation.text
 text >> second-operation.text
 first-operation.text >> first
 second-operation.text >> second
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring =
        expand_canonical_form_for_authoring(&checked, "twin-uppercase", &catalog).unwrap();
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
    host.run_external_form_to(
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

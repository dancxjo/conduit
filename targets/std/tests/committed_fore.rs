//! Public installed execution, not a hand-constructed delivery or kernel fixture.
use conduit_core::*;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    CommittedForeValue, ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter,
    StdHost, StdHostConfig, ThreadTimer,
};
use std::collections::BTreeMap;

fn prepared() -> (StdHost, Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let source = "plot uppercase (\n >> text: Text\n upper: Text >>\n) {\n operation: text/upper\n text >> operation.source\n operation.text >> upper\n}\n";
    planned(
        &startup,
        &profile,
        source,
        "uppercase",
        &[
            (PortDirection::Input, "text"),
            (PortDirection::Output, "upper"),
        ],
    )
}
fn planned(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    source: &str,
    plot: &str,
    ports: &[(PortDirection, &str)],
) -> (StdHost, Plan) {
    let checked = check_syntax_document(&parse_syntax_document(source), startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, plot, profile).unwrap();
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/committed-fore".into(),
        boot_id: "boot/committed-fore".into(),
        offer_generation: OfferGeneration(1),
    });
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = ports
        .iter()
        .copied()
        .map(|(direction, port)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port.into(),
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
    let lines = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: 64,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (host, plan)
}

struct Capture {
    expected: PlanFragment,
    calls: usize,
}
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        panic!("this consumer requires opaque kernel commit evidence")
    }
    fn deliver_committed(
        &mut self,
        committed: CommittedForeValue<'_>,
        legacy: ExternalForeDelivery,
    ) -> Result<(), String> {
        let scope = committed.scope();
        assert_eq!(scope.fragment, &self.expected);
        assert_eq!(
            *scope.play,
            bind_active_play(
                &self.expected.plan_id,
                &self.expected.host_id,
                &self.expected.boot_id,
                scope.play.play_sequence
            )
        );
        let fore = self
            .expected
            .fore_ports
            .iter()
            .find(|port| port.direction == PortDirection::Output)
            .unwrap();
        assert_eq!(scope.fore, fore);
        assert_eq!(scope.lowered.front_port_id, fore.front_port_id);
        assert_eq!(committed.sequence(), 0);
        assert_eq!(committed.bytes(), b"ACTUAL COMMIT");
        assert_eq!(committed.bytes(), legacy.bytes.as_slice());
        self.calls += 1;
        Ok(())
    }
}

#[test]
fn actual_installed_cord_commit_exposes_exact_borrowed_source_play_and_fore() {
    let (mut host, plan) = prepared();
    let mut capture = Capture {
        expected: plan.fragments[0].clone(),
        calls: 0,
    };
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "text".into(),
                track: ConnectionTrack::Payload,
                bytes: b"actual commit".to_vec(),
            }],
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert_eq!(capture.calls, 1);
    assert_eq!(report.external_fore_deliveries.len(), 1);
    assert_eq!(report.external_fore_deliveries[0].bytes, b"ACTUAL COMMIT");
}

#[test]
fn cancellation_before_source_commit_cannot_deliver_evidence() {
    let (mut host, plan) = prepared();
    let mut capture = Capture {
        expected: plan.fragments[0].clone(),
        calls: 0,
    };
    let control = conduit_std_host::RunControl::default();
    control
        .request_stop(conduit_std_host::RunControlRequestId::new("before-source").unwrap())
        .unwrap();
    let report = host
        .run_external_plot_controlled_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "text".into(),
                track: ConnectionTrack::Payload,
                bytes: b"actual commit".to_vec(),
            }],
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            &control,
        )
        .unwrap();
    assert_eq!(capture.calls, 0);
    assert!(report.external_fore_deliveries.is_empty());
}

#[test]
fn legacy_effect_adapter_keeps_its_existing_delivery_behavior() {
    struct Legacy(Vec<ExternalForeDelivery>);
    impl ExternalForeOutputAdapter for Legacy {
        fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
            self.0.push(output);
            Ok(())
        }
    }
    let (mut host, plan) = prepared();
    let mut legacy = Legacy(Vec::new());
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "text".into(),
                track: ConnectionTrack::Payload,
                bytes: b"actual commit".to_vec(),
            }],
            &mut legacy,
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert_eq!(legacy.0, report.external_fore_deliveries);
    assert_eq!(legacy.0.len(), 1);
    assert_eq!(legacy.0[0].bytes, b"ACTUAL COMMIT");
}

#[test]
fn recurring_flow_preserves_legacy_override_and_one_immutable_producer_scope() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    startup
        .insert_value_kind_alias("Tick", kind_id(conduit_time::TICK_VALUE_KIND))
        .unwrap();
    let source = "plot ticks (\n ticks: Tick...| >>\n) {\n clock: time/tick(count = 3, period-ms = 1)\n clock.tick >> ticks\n}\n";
    let (mut host, plan) = planned(
        &startup,
        &profile,
        source,
        "ticks",
        &[(PortDirection::Output, "ticks")],
    );
    struct Flow {
        sequences: Vec<u64>,
        scope: Option<(*const PlanFragment, *const PlannedForePort)>,
    }
    impl ExternalForeOutputAdapter for Flow {
        fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
            panic!("Flow must preserve its legacy borrowed override")
        }
        fn deliver_flow(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
            self.sequences.push(output.sequence);
            Ok(())
        }
        fn deliver_committed(
            &mut self,
            committed: CommittedForeValue<'_>,
            output: ExternalForeDelivery,
        ) -> Result<(), String> {
            let scope = committed.scope();
            assert_eq!(scope.fore.temporal, PortTemporal::Flow { closes: true });
            let addresses = (scope.fragment as *const _, scope.fore as *const _);
            if let Some(previous) = self.scope {
                assert_eq!(previous, addresses);
            }
            self.scope = Some(addresses);
            assert_eq!(committed.sequence(), self.sequences.len() as u64);
            assert_eq!(committed.bytes(), output.bytes.as_slice());
            assert!(std::mem::size_of_val(&committed) < 128);
            self.sequences.push(output.sequence);
            Ok(())
        }
    }
    let mut flow = Flow {
        sequences: Vec::with_capacity(3),
        scope: None,
    };
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[],
        &mut flow,
        &mut Vec::new(),
        &mut ThreadTimer,
    )
    .unwrap();
    assert_eq!(flow.sequences, [0, 1, 2]);
    struct LegacyFlow(Vec<u64>);
    impl ExternalForeOutputAdapter for LegacyFlow {
        fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
            panic!("default committed dispatch must preserve the Flow override")
        }
        fn deliver_flow(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
            self.0.push(output.sequence);
            Ok(())
        }
    }
    let mut legacy = LegacyFlow(Vec::with_capacity(3));
    host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[],
        &mut legacy,
        &mut Vec::new(),
        &mut ThreadTimer,
    )
    .unwrap();
    assert_eq!(legacy.0, [0, 1, 2]);
}

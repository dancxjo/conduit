//! Regression through an authored Plot, admitted std implementations and one
//! actual installed Play. No StepIo fixture supplies a second Current value.
use conduit_ai::*;
use conduit_core::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, RunControl,
    RunControlRequestId, StdHost, ThreadTimer,
};
use std::{collections::BTreeMap, sync::mpsc, time::Duration};
const SOURCE: &str = r#"plot temporal-current (
    >> windows: TemporalEvidenceWindow...|
    outcomes: TemporalEvidenceOutcome...| >>
) {
    intent: keep TemporalRetrievalIntent(latest_evidence(empty)) <= 4KiB for this play
    select: retrieval/temporal-select-flow
    windows >> select.window
    intent.current >> select.intent
    select.outcome >> outcomes
}
"#;
fn prepared() -> (StdHost, Plan) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_temporal_evidence_flow_catalog(&mut startup, &mut profile).unwrap();
    startup
        .insert(conduit_plot::KindSignature {
            kind: "state/latest".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let intent = TemporalRetrievalIntent::LatestEvidence;
    let ty = TemporalRetrievalIntent::semantic_type().unwrap();
    let initial = intent.into_structured().unwrap();
    conduit_semantic_catalog::state_value::install_state_value_kind(
        "TemporalRetrievalIntent",
        &ty,
        &initial,
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "temporal-current", &profile).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.capabilities.push(
        conduit_std_offers::state_value_std_offer("TemporalRetrievalIntent", &ty, &initial)
            .unwrap(),
    );
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "windows"),
        (PortDirection::Output, "outcomes"),
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
                byte_capacity: 65536,
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
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments.len(), 1);
    let placements = &plan.fragments[0].placements;
    assert_eq!(
        placements
            .iter()
            .filter(
                |p| p.kind_id.as_str() == conduit_semantic_catalog::state_value::STATE_VALUE_KIND
            )
            .count(),
        1
    );
    let selected = placements
        .iter()
        .find(|p| p.kind_id.as_str() == TEMPORAL_EVIDENCE_FLOW_KIND)
        .unwrap();
    assert_eq!(
        selected.implementation_id.as_str(),
        conduit_std_offers::TEMPORAL_EVIDENCE_FLOW_IMPLEMENTATION
    );
    assert_eq!(
        selected
            .inputs
            .iter()
            .find(|p| p.port_id.as_str() == "intent")
            .unwrap()
            .temporal,
        PortTemporal::Current
    );
    (host, plan)
}
fn window(identity: &str) -> Vec<u8> {
    let candidates = [("early", 10), ("late", 20)]
        .into_iter()
        .map(|(suffix, time)| TemporalEvidenceCandidate {
            identity: TemporalEvidenceIdentity::new(format!("{identity}/{suffix}")).unwrap(),
            provenance: TemporalProvenance {
                event_at: None,
                valid_from: None,
                valid_until: None,
                observed_at: Some(time),
                recorded_at: None,
                ingested_at: None,
                retrieved_at: 99,
                reference_at: 100,
                clock_basis: ClockBasis::UnixEpochMilliseconds,
                uncertainty_millis: None,
            },
            source: TemporalSource::Observed,
            boundary: None,
            transition: None,
            validity: TemporalValidity::Historical,
        });
    TemporalEvidenceWindow::new(
        TemporalEvidenceWindowCandidates::new(BoundedSequence::try_from_iter(candidates).unwrap())
            .unwrap(),
        false,
        TemporalEvidenceIdentity::new(identity.into()).unwrap(),
        TemporalReference {
            reference_at: 100,
            clock_basis: ClockBasis::UnixEpochMilliseconds,
        },
    )
    .unwrap()
    .encode()
    .unwrap()
}
struct Capture {
    outcomes: Vec<TemporalEvidenceOutcome>,
    control: RunControl,
}
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.track == ConnectionTrack::Payload {
            assert_eq!(delivery.front_port_id.as_str(), "outcomes");
            self.outcomes.push(
                TemporalEvidenceOutcome::decode(&delivery.bytes)
                    .map_err(|error| format!("{error:?}"))?,
            );
            if self.outcomes.len() == 2 {
                self.control
                    .request_stop(RunControlRequestId::new("two-windows-proved").unwrap())
                    .unwrap();
            }
        }
        Ok(())
    }
}
#[test]
fn one_authored_current_selects_two_windows_in_one_installed_play() {
    std::thread::Builder::new()
        .name("temporal-current-installed-proof".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(proof)
        .unwrap()
        .join()
        .unwrap();
}
fn proof() {
    let (mut host, plan) = prepared();
    let fragment = plan.fragments[0].clone();
    let inputs = ["window/one", "window/two"]
        .into_iter()
        .map(|identity| ExternalForeInput {
            front_port_id: "windows".into(),
            track: ConnectionTrack::Payload,
            bytes: window(identity),
        })
        .collect::<Vec<_>>();
    let control = RunControl::default();
    let mut capture = Capture {
        outcomes: Vec::with_capacity(2),
        control: control.clone(),
    };
    // A stalled old implementation must fail finitely. This thread only asks
    // the ordinary RunControl to stop; it never supplies data or steps work.
    let (finished, done) = mpsc::channel();
    let watchdog_control = control.clone();
    let watchdog = std::thread::spawn(move || {
        if done.recv_timeout(Duration::from_secs(10)).is_err() {
            let _ = watchdog_control.request_stop(
                RunControlRequestId::new("temporal-current-regression-timeout").unwrap(),
            );
        }
    });
    let report = host.run_external_plot_sequence_controlled_to(
        fragment.clone(),
        &inputs,
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
        &control,
    );
    let _ = finished.send(());
    watchdog.join().unwrap();
    let report = report.unwrap();
    assert_eq!(
        capture.outcomes.len(),
        2,
        "one Current must serve both windows in the same Play"
    );
    for (outcome, identity) in capture.outcomes.iter().zip(["window/one", "window/two"]) {
        assert_eq!(outcome.window_identity().get(), identity);
        let TemporalEvidenceFlowSelection::Selected(selected) = outcome.selection() else {
            panic!("latest evidence must be selected")
        };
        assert_eq!(selected.identities().get().len(), 1);
        assert_eq!(
            selected.identities().get().as_slice()[0].get(),
            &format!("{identity}/late")
        );
    }
    let kernel = report.kernel.expect("ordinary installed kernel Play");
    assert_eq!(kernel.active_play.plan_id, fragment.plan_id);
    assert_eq!(kernel.active_play.host_id, fragment.host_id);
    assert_eq!(kernel.active_play.boot_id, fragment.boot_id);
}

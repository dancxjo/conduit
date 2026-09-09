use conduit_core::{resource_offer, ObservationKind, TerminalDisposition, INPUT_RESOURCE_CLASS};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{
    hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
    RunControl, RunControlRequestId, StdHost, ThreadTimer,
};
use std::collections::{BTreeMap, VecDeque};

const SOURCE: &str = include_str!("../../../forms/desk-telegraph/main.conduit");
const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_record_temporal_catalogs(&mut startup, &mut profile).unwrap();
    conduit_net::install_ordered_record_queue_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    let checked = check_syntax_document(&syntax, &startup).expect("Desk Telegraph checks");
    expand_canonical_form(&checked, "desk_telegraph", &profile)
        .expect("Desk Telegraph recursively expands")
}

fn host() -> StdHost {
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement
        .capabilities
        .push(conduit_std_offers::hosted_keyboard_offer(
            "proof/desk-keyboard",
            "proof/desk-keyboard@1",
        ));
    advertisement.resources.push(resource_offer(
        "proof/desk-keyboard",
        INPUT_RESOURCE_CLASS,
        1,
    ));
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement.resources.sort();
    StdHost::from_advertisement(advertisement).unwrap()
}

fn plan(host: &StdHost, expanded: &conduit_form::ExpandedCanonicalForm) -> conduit_core::Plan {
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(expanded, &hosts).unwrap();
    let limits = expanded
        .connections
        .iter()
        .map(|connection| {
            let source_kind = expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == connection.source_gear_id)
                .map(|gear| gear.kind_id.as_str());
            let byte_capacity = match connection.value_kind.as_str() {
                conduit_human::KEY_EVENT_INFO_ID => conduit_human::KEY_EVENT_ENCODED_LEN as u32,
                conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND
                    if source_kind == Some(conduit_semantic_catalog::KEYMAP_KIND) =>
                {
                    4
                }
                conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND => {
                    conduit_text::MAX_TEXT_BYTES
                }
                _ => conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            };
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 4,
                    byte_capacity,
                },
            )
        })
        .collect();
    conduit_planner::plan_expanded_canonical_with_connection_limits(
        expanded,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: conduit_text::MAX_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap()
}

#[test]
fn canonical_desk_telegraph_frames_and_recovers_text_through_one_kernel_play() {
    let expanded = expanded();
    let kinds: Vec<_> = expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.as_str())
        .collect();
    for kind in [
        conduit_semantic_catalog::KEYBOARD_KIND,
        conduit_semantic_catalog::KEYMAP_KIND,
        conduit_semantic_catalog::TEXT_SUBMIT_LINES_KIND,
        conduit_net::TEXT_TO_TYPED_RECORD_KIND,
        conduit_net::TYPED_RECORD_FRAME_KIND,
        conduit_net::TYPED_RECORD_DEFRAME_KIND,
        conduit_net::TYPED_RECORD_TO_TEXT_KIND,
        conduit_net::ORDERED_RECORD_QUEUE_KIND,
        "presentation/text",
    ] {
        assert!(kinds.contains(&kind), "missing expanded {kind}");
    }

    let mut host = host();
    let plan = plan(&host, &expanded);
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 9);
    let plan_id = plan.plan_id.clone();

    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    struct Keyboard {
        events: VecDeque<[u8; 3]>,
        stop: Option<RunControl>,
        idle_polls: usize,
    }
    impl HostedKeyboardAdapter for Keyboard {
        fn poll_next(&mut self) -> HostedKeyboardPoll {
            if let Some(bytes) = self.events.pop_front() {
                return HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&bytes).unwrap());
            }
            if self.idle_polls < 128 {
                self.idle_polls += 1;
                return HostedKeyboardPoll::Pending;
            }
            if let Some(control) = self.stop.take() {
                control
                    .request_stop(RunControlRequestId::new("stop-desk-telegraph").unwrap())
                    .unwrap();
            }
            HostedKeyboardPoll::Pending
        }
    }
    let control = RunControl::default();
    let mut keyboard = Keyboard {
        events: [
            [6, 0, 0],
            [4, 0, 0],
            [15, 0, 0],
            [15, 0, 0],
            [12, 0, 0],
            [17, 0, 0],
            [10, 0, 0],
            [40, 0, 0],
        ]
        .into(),
        stop: Some(control.clone()),
        idle_polls: 0,
    };
    let report = host
        .run_fragment_controlled_with_keyboard_to(
            plan.fragments[0].clone(),
            &mut output,
            &mut timer,
            &control,
            Some(&mut keyboard),
        )
        .expect("Desk Telegraph executes through the production kernel");
    assert!(String::from_utf8_lossy(&output).contains("calling\n"));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Cancelled { .. }
        })
    ));
    let kernel = report.kernel.expect("kernel execution evidence exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    println!(
        "{EVIDENCE_MARKER}{{\"plan_id\":\"{}\",\"play_id\":\"{}\"}}",
        plan_id.as_str(),
        kernel.active_play_id.as_str()
    );
}

#[test]
fn missing_or_stale_codec_realization_refuses_before_presentation() {
    let expanded = expanded();
    let mut host = host();
    let mut plan = plan(&host, &expanded);
    let frame = plan.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_net::TYPED_RECORD_FRAME_KIND)
        .unwrap();
    frame.artifact_id = "wrong/typed-record-codec@1".into();

    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    assert!(host
        .run_fragment_to(plan.fragments.remove(0), &mut output, &mut timer)
        .is_err());
    assert!(!String::from_utf8_lossy(&output).contains("CALLING"));
}

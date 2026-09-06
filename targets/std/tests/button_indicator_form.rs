//! Deterministic native execution, not a physical keyboard or indicator claim.
use conduit_core::{resource_offer, ObservationKind, TerminalDisposition, INPUT_RESOURCE_CLASS};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{
    hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
    RunControl, StdHost, TimerAdapter,
};
use std::{collections::VecDeque, time::Duration};

const SOURCE: &str = include_str!("../../../forms/button-across-room/main.conduit");
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: Duration) {}
}
struct Keyboard(VecDeque<[u8; 3]>);
impl HostedKeyboardAdapter for Keyboard {
    fn poll_next(&mut self) -> HostedKeyboardPoll {
        self.0
            .pop_front()
            .map_or(HostedKeyboardPoll::Cancelled, |bytes| {
                HostedKeyboardPoll::Event(conduit_human::KeyEvent::decode(&bytes).unwrap())
            })
    }
}

#[test]
fn unchanged_canonical_form_runs_on_native_kernel() {
    run_form(SOURCE, 2);
}

#[test]
fn maximum_button_bound_is_not_limited_by_the_unrelated_toggle_sink() {
    let source = SOURCE.replace("input/button\n", "input/button(8)\n");
    assert_ne!(source, SOURCE);
    run_form(&source, 8);
}

fn run_form(source: &str, transitions: usize) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "button_across_room", &profile).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.capabilities = vec![
        conduit_std_offers::button::offer(),
        conduit_std_offers::button::mapper_offer(),
        conduit_std_offers::button::indicator_offer(),
    ];
    advertisement
        .resources
        .push(resource_offer("proof/keyboard", INPUT_RESOURCE_CLASS, 1));
    advertisement.resources.sort();
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let limits = expanded
        .connections
        .iter()
        .map(|connection| {
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if connection.value_kind.as_str() == conduit_core::BOOL_INFO_ID {
                        1
                    } else {
                        conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
                    },
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &Default::default(),
            line_candidates: &Default::default(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    let mut events = VecDeque::from([[4, 0, 0]]);
    events.extend((0..transitions).map(|sequence| [0x2c, (sequence % 2) as u8, 0]));
    let mut keyboard = Keyboard(events);
    let mut output = Vec::new();
    let report = host
        .run_fragment_controlled_with_keyboard_to(
            plan.fragments[0].clone(),
            &mut output,
            &mut Timer,
            &RunControl::default(),
            Some(&mut keyboard),
        )
        .unwrap();
    let output = String::from_utf8(output).unwrap();
    let states = output
        .lines()
        .filter_map(|line| line.strip_prefix("bool value="))
        .collect::<Vec<_>>();
    let expected = (0..transitions)
        .map(|sequence| if sequence % 2 == 0 { "true" } else { "false" })
        .collect::<Vec<_>>();
    assert_eq!(states, expected, "{output}");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    for (events, expected) in [
        (vec![], "Cancelled"),
        (vec![[0x2c, 1, 0]], "InvalidInput"),
        (vec![[0x2c, 0, 0], [0x2c, 0, 0]], "InvalidInput"),
    ] {
        let mut input = Keyboard(events.into());
        let failure = host
            .run_fragment_controlled_with_keyboard_to(
                plan.fragments[0].clone(),
                &mut Vec::new(),
                &mut Timer,
                &RunControl::default(),
                Some(&mut input),
            )
            .unwrap_err();
        assert!(failure.contains(expected), "{failure}");
    }
    let missing = host
        .run_fragment_controlled_with_keyboard_to(
            plan.fragments[0].clone(),
            &mut Vec::new(),
            &mut Timer,
            &RunControl::default(),
            None,
        )
        .unwrap_err();
    assert!(missing.contains("no admitted Host adapter"), "{missing}");
}

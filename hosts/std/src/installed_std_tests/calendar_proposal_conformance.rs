use super::{host, installed_std, RecordingTimer};
use conduit_core::{BaseImplementationId, ConfigurationValue, PortDirection, PortTemporal};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use conduit_time::{
    AvailabilityBasis, AvailabilityInterval, AvailabilityState, MeetingCandidate,
    MeetingProposalRequest, NamedTimeZone, ParticipantAvailability, TemporalBoundary,
    TemporalInstant, TemporalScale, TemporalWindow,
};
use std::collections::BTreeMap;

const SINK: &str = installed_std::test_structured_selector::SINK_KIND;

#[test]
fn checked_calendar_request_prepares_then_emits_three_inert_candidates() {
    let fixture = fixture();
    let expected = fixture
        .request
        .propose(&fixture.availability)
        .expect("fixture has common availability");
    assert_eq!(expected.candidates.len(), 3);
    assert_eq!(expected.rejected.len(), 1);
    assert_eq!(
        expected.candidates[1].tentative_participants,
        ["participant/bob"]
    );
    assert_eq!(
        expected.rejected[0].conflicts[0].state,
        AvailabilityState::Busy
    );
    let encoded = installed_std::calendar_proposal_encoding::encode(&expected).unwrap();
    let source = source(&fixture, &hex(&encoded));
    let (startup, profile, sink_offer) = catalogs();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).expect("calendar Form checks");
    let expanded =
        expand_canonical_form(&checked, "calendar-proof", &profile).expect("calendar Form expands");

    let mut advertisement = host("calendar-proposal-host").advertisement().clone();
    advertisement.capabilities.push(sink_offer);
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("calendar Form plans without authority or resource grants");
    let planned = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::CALENDAR_PROPOSAL_KIND)
        .unwrap();
    assert!(matches!(
        planned.configuration[0].value,
        ConfigurationValue::Structured(_)
    ));
    assert!(planned.host_operations.is_empty());
    assert!(planned.resources.is_empty());
    assert!(planned.authority.is_empty());

    let mut output = Vec::with_capacity(2_048);
    let mut timer = RecordingTimer { waits: vec![] };
    let mut sign_sequence = 0;
    let report = installed_std::run_fragment(
        installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: None,
        },
        &plan.fragments[0],
        0,
        &mut sign_sequence,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .expect("calendar proposal executes through the production kernel");
    let kernel = report.kernel.expect("kernel evidence exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert!(timer.waits.is_empty());
}

#[test]
fn stale_missing_and_over_profile_calendar_requests_refuse_before_play() {
    let variants = [
        (999, true, 3, false, "stale availability"),
        (2_000, false, 3, false, "missing participant"),
        (2_000, true, 4, false, "over-profile result count"),
        (2_000, true, 3, true, "malformed temporal scale"),
    ];
    for (usable_until, include_bob, maximum_results, malformed_scale, label) in variants {
        let mut fixture = fixture();
        let baseline = fixture
            .request
            .propose(&fixture.availability)
            .expect("baseline fixture has common availability");
        let expected = installed_std::calendar_proposal_encoding::encode(&baseline).unwrap();
        fixture.request.maximum_results = maximum_results;
        for availability in &mut fixture.availability {
            availability.basis.usable_until = instant(usable_until);
        }
        if !include_bob {
            fixture.availability.pop();
        }
        let mut source = source(&fixture, &hex(&expected));
        if malformed_scale {
            source = source.replacen("start_scale: \"seconds\"", "start_scale: \"fortnights\"", 1);
        }
        let (startup, profile, sink_offer) = catalogs();
        let syntax = parse_syntax_document(&source);
        assert!(
            syntax.diagnostics.is_empty(),
            "{label}: {:?}",
            syntax.diagnostics
        );
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        let expanded = expand_canonical_form(&checked, "calendar-proof", &profile).unwrap();
        let mut advertisement = host(&format!("calendar-refusal-{maximum_results}"))
            .advertisement()
            .clone();
        advertisement.capabilities.push(sink_offer);
        advertisement
            .capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        let hosts = [advertisement.clone()];
        let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
        let plan = conduit_planner::plan_expanded_canonical_with_options(
            &expanded,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
        let mut output = Vec::new();
        let mut timer = RecordingTimer { waits: vec![] };
        let mut sign_sequence = 0;
        let result = installed_std::run_fragment(
            installed_std::InstalledRunHost {
                advertisement: &advertisement,
                playback: None,
                midi_input: None,
                midi_output: None,
                keyboard: None,
                local_model: None,
                vector_search: None,
                calendar: None,
            },
            &plan.fragments[0],
            0,
            &mut sign_sequence,
            &mut output,
            &mut timer,
            &crate::RunControl::default(),
        );
        let error = result.expect_err(&format!("{label} unexpectedly reached Play"));
        assert!(
            error.contains("calendar"),
            "{label} failed through an unrelated operation: {error}"
        );
        assert!(output.is_empty(), "{label} emitted partial output");
        assert!(timer.waits.is_empty(), "{label} performed timed work");
    }
}

struct Fixture {
    request: MeetingProposalRequest,
    availability: Vec<ParticipantAvailability>,
}

fn fixture() -> Fixture {
    let candidate = |identity: &str, start: u64| MeetingCandidate {
        identity: identity.into(),
        interval: window(start, start + 60),
        rationale: format!("candidate beginning at {start}"),
    };
    let candidates = vec![
        candidate("candidate/one", 1_100),
        candidate("candidate/two", 1_200),
        candidate("candidate/three", 1_300),
        candidate("candidate/conflict", 1_400),
    ];
    let participant =
        |identity: &str, zone: &str, states: [AvailabilityState; 4]| ParticipantAvailability {
            participant_identity: identity.into(),
            zone: NamedTimeZone::new(zone.to_string(), "tzdb/2026a".to_string()).unwrap(),
            basis: AvailabilityBasis {
                identity: format!("availability/{identity}"),
                observed_at: instant(900),
                usable_until: instant(2_000),
            },
            intervals: candidates
                .iter()
                .zip(states)
                .map(|(candidate, state)| AvailabilityInterval {
                    participant_identity: identity.into(),
                    interval: candidate.interval.clone(),
                    state,
                })
                .collect(),
        };
    let availability = vec![
        participant(
            "participant/alice",
            "America/Los_Angeles",
            [AvailabilityState::Free; 4],
        ),
        participant(
            "participant/bob",
            "Europe/London",
            [
                AvailabilityState::Free,
                AvailabilityState::Tentative,
                AvailabilityState::Free,
                AvailabilityState::Busy,
            ],
        ),
    ];
    Fixture {
        request: MeetingProposalRequest {
            identity: "proposal/cross-timezone".into(),
            reference_at: instant(1_000),
            participant_identities: vec!["participant/alice".into(), "participant/bob".into()],
            candidates,
            maximum_results: 3,
        },
        availability,
    }
}

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "utc".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn window(start: u64, end: u64) -> TemporalWindow {
    TemporalWindow::new(
        instant(start),
        TemporalBoundary::Inclusive,
        instant(end),
        TemporalBoundary::Exclusive,
    )
    .unwrap()
}

fn source(fixture: &Fixture, expected_hex: &str) -> String {
    let participants = slots(
        fixture
            .request
            .participant_identities
            .iter()
            .map(|identity| format!("participant(\"{identity}\")"))
            .collect(),
        conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
    );
    let candidates = slots(
        fixture
            .request
            .candidates
            .iter()
            .map(|candidate| {
                format!(
                    "candidate({{ identity: \"{}\", interval: {}, rationale: \"{}\" }})",
                    candidate.identity,
                    window_source(&candidate.interval),
                    candidate.rationale
                )
            })
            .collect(),
        conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_CANDIDATES,
    );
    let availability = slots(
        fixture
            .availability
            .iter()
            .map(|participant| {
                let intervals = slots(
                    participant
                        .intervals
                        .iter()
                        .map(|interval| {
                            format!(
                                "interval({{ {}, participant_identity: \"{}\", state: \"{}\" }})",
                                flattened_window_source(&interval.interval),
                                interval.participant_identity,
                                state_name(interval.state)
                            )
                        })
                        .collect(),
                    conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_INTERVALS,
                );
                format!(
                    "participant({{ basis_identity: \"{}\", intervals: [{}], observed_at: {}, participant_identity: \"{}\", usable_until: {}, zone: \"{}\", zone_rule_set: \"{}\" }})",
                    participant.basis.identity,
                    intervals,
                    instant_source(&participant.basis.observed_at),
                    participant.participant_identity,
                    instant_source(&participant.basis.usable_until),
                    participant.zone.identity(),
                    participant.zone.rule_set(),
                )
            })
            .collect(),
        conduit_std_catalog::CALENDAR_PROPOSAL_MAXIMUM_PARTICIPANTS,
    );
    format!(
        "form calendar-proof {{\n  propose: calendar/propose-meeting({{ availability: [{availability}], candidates: [{candidates}], identity: \"{}\", maximum_results: {}, participant_identities: [{participants}], reference_at: {} }})\n  sink: {SINK}(value = \"{expected_hex}\")\n  propose.proposal > sink.input\n}}\n",
        fixture.request.identity,
        fixture.request.maximum_results,
        instant_source(&fixture.request.reference_at),
    )
}

fn slots(mut active: Vec<String>, maximum: u16) -> String {
    while active.len() < usize::from(maximum) {
        active.push("unused(\"\")".into());
    }
    active.join(", ")
}

fn instant_source(value: &TemporalInstant) -> String {
    format!(
        "{{ basis: \"{}\", resolution_ticks: {}, scale: \"seconds\", ticks: {}, uncertainty_ticks: {} }}",
        value.clock_basis, value.resolution_ticks, value.ticks, value.uncertainty_ticks
    )
}

fn window_source(value: &TemporalWindow) -> String {
    format!(
        "{{ end: {}, start: {} }}",
        instant_source(value.end()),
        instant_source(value.start())
    )
}

fn flattened_window_source(value: &TemporalWindow) -> String {
    format!(
        "end_basis: \"{}\", end_boundary: \"{}\", end_resolution_ticks: {}, end_scale: \"{}\", end_ticks: {}, end_uncertainty_ticks: {}, start_basis: \"{}\", start_boundary: \"{}\", start_resolution_ticks: {}, start_scale: \"{}\", start_ticks: {}, start_uncertainty_ticks: {}",
        value.end().clock_basis,
        boundary_name(value.end_boundary()),
        value.end().resolution_ticks,
        scale_name(value.end().scale),
        value.end().ticks,
        value.end().uncertainty_ticks,
        value.start().clock_basis,
        boundary_name(value.start_boundary()),
        value.start().resolution_ticks,
        scale_name(value.start().scale),
        value.start().ticks,
        value.start().uncertainty_ticks,
    )
}

fn boundary_name(value: TemporalBoundary) -> &'static str {
    match value {
        TemporalBoundary::Inclusive => "inclusive",
        TemporalBoundary::Exclusive => "exclusive",
    }
}

fn scale_name(value: TemporalScale) -> &'static str {
    match value {
        TemporalScale::Seconds => "seconds",
        TemporalScale::Milliseconds => "milliseconds",
        TemporalScale::Microseconds => "microseconds",
        TemporalScale::Nanoseconds => "nanoseconds",
    }
}

fn state_name(value: AvailabilityState) -> &'static str {
    match value {
        AvailabilityState::Free => "free",
        AvailabilityState::Tentative => "tentative",
        AvailabilityState::Busy => "busy",
        AvailabilityState::Unavailable => "unavailable",
    }
}

fn catalogs() -> (
    StartupCatalog,
    ProfileCatalog,
    conduit_core::CapabilityOffer,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_calendar_proposal_catalogs(&mut startup, &mut profile).unwrap();
    let value_type = conduit_std_catalog::calendar_proposal_result_type();
    let mut sink_offer =
        installed_std::test_structured_selector::offer(&value_type, PortDirection::Input);
    sink_offer.inputs[0].temporal = PortTemporal::Value;
    startup
        .insert(KindSignature {
            kind: SINK.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: Some("\"\"".into()),
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: sink_offer.kind_id.clone(),
            kind_contract_revision: sink_offer.kind_contract_revision.clone(),
            inputs: sink_offer.inputs.clone(),
            outputs: vec![],
            configuration: vec![ConfigurationField {
                key: "value".into(),
                default_value: ConfigurationValue::Text(String::new()),
                validation: ConfigurationRule::TextBytes {
                    maximum: (conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES * 2) as u32,
                },
            }],
        })
        .unwrap();
    (startup, profile, sink_offer)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

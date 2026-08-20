use conduit_ai::{
    interpret_temporal_proposal, llm_contract, ConfidencePermille, LlmDeterminismProfile,
    ModelDerivedResult, ModelEffectProposal, ModelResultDisposition, ModelResultProvenance,
    ModelWorkAccounting, PreferredLocalWindow, ProposalDecisionOutcome, ProposalGate,
    ProposalRefusal, RelativeDateWindow, TemporalAmbiguity, TemporalInterpretationRefusal,
    TemporalInterpretationRequest, TemporalProposal, TemporalProposalProvenance,
    TemporalResolutionTruth, LLM_INTERPRET_KIND, MAXIMUM_RECURRENCE_OCCURRENCES,
};
use conduit_core::{
    AvailabilityBasis, AvailabilityInterval, AvailabilityState, KindId, LocalDate, LocalDateTime,
    LocalTime, MeetingProposalRefusal, NamedTimeZone, ParticipantAvailability, PlanId,
    TemporalBoundary, TemporalInstant, TemporalScale, TemporalWindow, ZonedResolution,
    UNIX_UTC_CLOCK_BASIS,
};

#[test]
fn model_proposal_resolves_bounded_next_week_candidates_before_fresh_availability() {
    let request = request();
    let result = model_result("provider/a");
    let proposal = interpret_temporal_proposal(&request, &result, proposal_for(&result)).unwrap();

    let resolved = proposal.resolve(&request, &truth()).unwrap();
    assert_eq!(resolved.participant_identities, ["person/alex"]);
    assert_eq!(resolved.candidates.len(), 4);
    assert!(resolved
        .candidates
        .iter()
        .all(|candidate| !candidate.identity.ends_with("/5")));

    // Availability is newer deterministic truth joined only after model interpretation.
    let availability = fresh_availability(&request, &resolved);
    let meeting = resolved.propose(&availability).unwrap();
    assert_eq!(meeting.candidates.len(), 3);
    assert_eq!(meeting.availability_basis_identities, ["free-busy/alex/42"]);
}

#[test]
fn explicit_reference_instant_zone_and_rule_set_must_match_resolution_truth() {
    let request = request();
    let mut supplied = truth();
    supplied.reference = unique(
        local(2026, 8, 20, 9, 0),
        NamedTimeZone::new("America/New_York".into(), "tzdb/2026b".into()).unwrap(),
        wall(1_000_000),
    );
    assert_eq!(
        proposal().resolve(&request, &supplied),
        Err(TemporalInterpretationRefusal::ReferenceResolutionMismatch)
    );

    let mut uncertain = request;
    uncertain.reference_at.uncertainty_ticks = 1;
    assert_eq!(
        proposal().resolve(&uncertain, &truth()),
        Err(TemporalInterpretationRefusal::InvalidRequest)
    );
}

#[test]
fn gap_fold_and_timezone_abbreviation_remain_distinct_refusals() {
    let request = request();
    let mut ambiguous = truth();
    let local = local(2026, 8, 24, 13, 0);
    ambiguous.candidate_starts[0] = ZonedResolution::Ambiguous {
        local,
        zone: zone(),
        earlier: wall(1_100_000),
        later: wall(1_103_600),
    };
    assert_eq!(
        proposal().resolve(&request, &ambiguous),
        Err(TemporalInterpretationRefusal::AmbiguousCivilTime)
    );

    let mut nonexistent = truth();
    nonexistent.candidate_starts[0] = ZonedResolution::Nonexistent {
        local,
        zone: zone(),
        gap_before: wall(1_100_000),
        gap_after: wall(1_103_600),
    };
    assert_eq!(
        proposal().resolve(&request, &nonexistent),
        Err(TemporalInterpretationRefusal::NonexistentCivilTime)
    );

    let mut abbreviation = proposal();
    abbreviation.unresolved_ambiguities =
        vec![TemporalAmbiguity::TimeZoneAbbreviation("PST".into())];
    assert_eq!(
        abbreviation.resolve(&request, &truth()),
        Err(TemporalInterpretationRefusal::UnresolvedAmbiguity)
    );
}

#[test]
fn malformed_local_time_is_rejected_as_typed_data_before_resolution() {
    let mut value = serde_json::to_value(proposal()).unwrap();
    value["preferred_local_window"]["start"]["hour"] = 25.into();
    let malformed: TemporalProposal = serde_json::from_value(value).unwrap();
    assert_eq!(
        malformed.resolve(&request(), &truth()),
        Err(TemporalInterpretationRefusal::InvalidProposal)
    );
}

#[test]
fn all_named_model_and_freshness_negative_cases_are_machine_readable() {
    let request = request();

    let mut unknown = proposal();
    unknown.participant_refs = vec!["person/nonexistent".into()];
    assert_eq!(
        unknown.resolve(&request, &truth()),
        Err(TemporalInterpretationRefusal::UnknownParticipant)
    );

    let mut duration = proposal();
    duration.duration_minutes = 0;
    assert_eq!(
        duration.resolve(&request, &truth()),
        Err(TemporalInterpretationRefusal::MalformedDuration)
    );

    let mut recurrence = proposal();
    recurrence.recurrence_occurrences = Some(MAXIMUM_RECURRENCE_OCCURRENCES + 1);
    assert_eq!(
        recurrence.resolve(&request, &truth()),
        Err(TemporalInterpretationRefusal::OverBroadRecurrence)
    );

    let mut hallucinated = proposal();
    hallucinated.claimed_existing_event_refs = vec!["event/not-observed".into()];
    assert_eq!(
        hallucinated.resolve(&request, &truth()),
        Err(TemporalInterpretationRefusal::HallucinatedExistingEvent)
    );

    let resolved = proposal().resolve(&request, &truth()).unwrap();
    let mut stale = fresh_availability(&request, &resolved);
    stale[0].basis.usable_until.ticks = request.reference_at.ticks - 1;
    assert_eq!(
        resolved.propose(&stale),
        Err(MeetingProposalRefusal::StaleAvailability)
    );
}

#[test]
fn temporal_suggestion_has_no_calendar_write_authority() {
    let effect = ModelEffectProposal {
        proposal_id: "proposal/calendar-write".into(),
        plan_id: PlanId::from("plan/temporal-candidate"),
        operation_kind: KindId::from("calendar/create-event"),
        canonical_arguments: vec![1, 2, 3],
        rationale: "create the selected candidate".into(),
        evidence: vec![],
    };
    let mut gate = ProposalGate::new(None, 1).unwrap();
    let disposition = gate.submit(effect).unwrap();
    assert_eq!(
        disposition.decision.outcome,
        ProposalDecisionOutcome::Refused(ProposalRefusal::MissingAuthority)
    );
    assert!(disposition.request.is_none());
}

#[test]
fn provider_replacement_changes_provenance_not_portable_proposal_semantics() {
    let first_result = model_result("provider/a");
    let second_result = model_result("provider/b");
    let first = interpret_temporal_proposal(&request(), &first_result, proposal_for(&first_result))
        .unwrap();
    let second =
        interpret_temporal_proposal(&request(), &second_result, proposal_for(&second_result))
            .unwrap();

    let first_resolved = first.resolve(&request(), &truth()).unwrap();
    let second_resolved = second.resolve(&request(), &truth()).unwrap();
    assert_eq!(first_resolved, second_resolved);
    assert_ne!(first.provenance, second.provenance);

    let mut unrelated = model_result("provider/a");
    unrelated.payload[0] ^= 1;
    assert_eq!(
        interpret_temporal_proposal(&request(), &unrelated, proposal_for(&unrelated)),
        Err(TemporalInterpretationRefusal::InvalidModelEnvelope)
    );
}

fn request() -> TemporalInterpretationRequest {
    TemporalInterpretationRequest {
        identity: "interpretation/alex-next-week".into(),
        natural_language:
            "Find a 30-minute time with Alex next week, preferably my afternoon, but not Tuesday."
                .into(),
        reference_at: wall(1_000_000),
        reference_zone: zone(),
        participant_directory: vec!["person/alex".into()],
        maximum_candidates: 8,
        maximum_results: 3,
    }
}

fn proposal() -> TemporalProposal {
    proposal_for(&model_result("provider/a"))
}

fn proposal_for(result: &ModelDerivedResult) -> TemporalProposal {
    TemporalProposal {
        identity: "proposal/alex-next-week".into(),
        date_window: RelativeDateWindow {
            start_day_offset: 4,
            end_day_offset: 8,
        },
        duration_minutes: 30,
        preferred_local_window: PreferredLocalWindow {
            start: LocalTime::new(13, 0, 0, 0).unwrap(),
            end: LocalTime::new(17, 0, 0, 0).unwrap(),
        },
        excluded_day_offsets: vec![5],
        participant_refs: vec!["person/alex".into()],
        unresolved_ambiguities: vec![],
        recurrence_occurrences: None,
        claimed_existing_event_refs: vec![],
        provenance: TemporalProposalProvenance {
            source: ModelResultProvenance::ModelDerived,
            implementation_identity: result.implementation_identity.clone(),
            request_identity: result.request_identity.clone(),
            run_identity: result.run_identity.clone(),
        },
    }
}

fn truth() -> TemporalResolutionTruth {
    TemporalResolutionTruth {
        reference: unique(local(2026, 8, 20, 9, 0), zone(), wall(1_000_000)),
        candidate_starts: [4_i16, 6, 7, 8]
            .into_iter()
            .map(|offset| {
                unique(
                    local(2026, 8, 20 + u8::try_from(offset).unwrap(), 13, 0),
                    zone(),
                    wall(1_000_000 + u64::try_from(offset).unwrap() * 86_400 + 14_400),
                )
            })
            .collect(),
    }
}

fn fresh_availability(
    request: &TemporalInterpretationRequest,
    resolved: &conduit_core::MeetingProposalRequest,
) -> Vec<ParticipantAvailability> {
    vec![ParticipantAvailability {
        participant_identity: "person/alex".into(),
        zone: zone(),
        basis: AvailabilityBasis {
            identity: "free-busy/alex/42".into(),
            observed_at: wall(request.reference_at.ticks - 10),
            usable_until: wall(request.reference_at.ticks + 1_000_000),
        },
        intervals: resolved
            .candidates
            .iter()
            .map(|candidate| AvailabilityInterval {
                participant_identity: "person/alex".into(),
                interval: TemporalWindow::new(
                    candidate.interval.start().clone(),
                    TemporalBoundary::Inclusive,
                    candidate.interval.end().clone(),
                    TemporalBoundary::Inclusive,
                )
                .unwrap(),
                state: AvailabilityState::Free,
            })
            .collect(),
    }]
}

fn model_result(implementation: &str) -> ModelDerivedResult {
    let contract = llm_contract(LLM_INTERPRET_KIND).unwrap();
    let mut result = ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().into(),
        payload: Vec::new(),
        implementation_identity: implementation.into(),
        request_identity: "request/temporal/1".into(),
        run_identity: "run/temporal/1".into(),
        confidence: Some(ConfidencePermille(800)),
        disposition: ModelResultDisposition::Produced,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
        accounting: ModelWorkAccounting {
            input_bytes: 100,
            context_items: 1,
            output_bytes: 0,
            work_units: 100,
            history_items: 0,
        },
    };
    result.payload = proposal_for(&result).canonical_semantic_payload();
    result.accounting.output_bytes = result.payload.len() as u64;
    result
}

fn zone() -> NamedTimeZone {
    NamedTimeZone::new("America/Los_Angeles".into(), "tzdb/2026b".into()).unwrap()
}

fn local(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> LocalDateTime {
    LocalDateTime::new(
        LocalDate::new(year, month, day).unwrap(),
        LocalTime::new(hour, minute, 0, 0).unwrap(),
    )
}

fn unique(local: LocalDateTime, zone: NamedTimeZone, instant: TemporalInstant) -> ZonedResolution {
    ZonedResolution::Unique {
        local,
        zone,
        instant,
    }
}

fn wall(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: UNIX_UTC_CLOCK_BASIS.into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

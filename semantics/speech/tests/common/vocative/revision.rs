//! Exact supplied dependency and prosody commitment fixture.
use super::*;
pub(super) fn text(s: &str) -> RevisionText<'_> {
    RevisionText::new(s).unwrap()
}
pub(super) fn with_current(
    dependency: &PreparedCommittedDependency,
    lexical: &PreparedLexicalTape,
    run: impl FnOnce(&CurrentCommittedDependency<'_, '_>) -> Vec<u8>,
) {
    let domain = DependencyRevisions {
        source_text: lexical.tape().source().material(),
        tokens: 3,
    };
    let context = RevisionContext {
        stream: text("vocative-analysis"),
        subject: text("vocative-fixture"),
        epoch: text("1"),
        producer: text("supplied-graph"),
        policy: text("vocative@1"),
    };
    let reference = |sequence, label| RevisionReference {
        context,
        sequence,
        event: text(label),
    };
    let evidence = [RevisionEvidence {
        source: text("supplied-graph"),
        generation: text("1"),
    }];
    let delta = DependencyDelta::Assert(dependency.arc().clone());
    let proposed = RevisionEvent::new(
        &domain,
        reference(1, "proposed"),
        &evidence,
        RevisionChange::Proposed { delta: &delta },
    )
    .unwrap();
    let stable = RevisionEvent::new(
        &domain,
        reference(2, "stable"),
        &evidence,
        RevisionChange::Stable {
            revision: proposed.reference(),
            through: TokenFrontier(3),
        },
    )
    .unwrap();
    let commit = RevisionEvent::new(
        &domain,
        reference(3, "commit"),
        &evidence,
        RevisionChange::Committed {
            revision: proposed.reference(),
            through: TokenFrontier(3),
        },
    )
    .unwrap();
    let correction = RevisionEvent::new(
        &domain,
        reference(4, "correction"),
        &evidence,
        RevisionChange::Corrected {
            commit: commit.reference(),
            delta: &delta,
            reason: text("later-arrival"),
        },
    )
    .unwrap();
    let limits = RevisionLimits {
        history_events: 8,
        revisable_units: 4,
    };
    let mut session = AnalysisSession::new(&domain, context, limits).unwrap();
    // An equal-context session without this committed journal cannot publish it.
    let foreign = AnalysisSession::new(&domain, context, limits).unwrap();
    assert!(foreign.current(proposed.reference(), dependency).is_err());
    session.append(&proposed, None, false, false).unwrap();
    session.append(&stable, None, false, false).unwrap();
    session
        .append(&commit, Some(dependency), false, false)
        .unwrap();
    let current = session.current(proposed.reference(), dependency).unwrap();
    let played = run(&current);
    let original = played.clone();
    session.append(&correction, None, false, false).unwrap();
    assert!(matches!(
        session.current(proposed.reference(), dependency),
        Err(AnalysisSessionRefusal::Corrected)
    ));
    assert_eq!(played, original);
}
pub(super) fn with_subject(
    request: &LanguageParserBeamCommitRequest,
    subject: usize,
) -> LanguageParserBeamCommitRequest {
    let candidate = |value: &LanguageParserHypothesis| {
        let state = value.state();
        let mut relations = [
            state.relation0().clone(),
            state.relation1().clone(),
            state.relation2().clone(),
            state.relation3().clone(),
        ];
        relations[subject] = LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Nsubj,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap();
        let state = LanguageParserState::new(
            state.basis().clone(),
            *state.committed(),
            *state.depth(),
            *state.heads(),
            relations[0].clone(),
            relations[1].clone(),
            relations[2].clone(),
            relations[3].clone(),
            *state.stack(),
            *state.token_count(),
            *state.unread(),
        )
        .unwrap();
        LanguageParserHypothesis::new(*value.active(), *value.identity(), *value.score(), state)
            .unwrap()
    };
    LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            request.beam().basis().clone(),
            candidate(request.beam().candidate0()),
            candidate(request.beam().candidate1()),
            candidate(request.beam().candidate2()),
            candidate(request.beam().candidate3()),
        )
        .unwrap(),
        false,
        request.frontier().clone(),
        false,
        *request.through(),
    )
    .unwrap()
}
pub(super) fn binding(
    f: &fixture::prosody::Fixture,
    rich: &PreparedRichProsody,
    hz: u64,
) -> SpeechLinguisticProsodyBinding {
    SpeechLinguisticProsodyBinding::new(
        rich.accepted().choice().clone(),
        "authored/vocative-neutral-extras@1".into(),
        f.profile.language().clone(),
        f.profile.identity().clone(),
        carrier::inventory::provenance(),
        SpeechLinguisticProsodyRealization::new(
            SpeechDurationSpecification::known(100, 3).unwrap(),
            SpeechBoundarySpecification::known(SpeechBoundaryKind::Phrase).unwrap(),
            SpeechSegmentProsodyIntent::new(
                SpeechDurationSpecification::known(100, 10).unwrap(),
                SpeechCycleSpecification::known(hz, 1).unwrap(),
                SpeechIntensitySpecification::known(5, 4).unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

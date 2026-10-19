//! Correctly classified synthetic ASR snapshots, not model recognition evidence.
//! The consumer is the existing Language lexical/parser/analysis spine. ASR
//! finality never supplies syntactic commitment or physical playback authority.
#![cfg(feature = "semantic-bindings")]
#[path = "common/asr_common_language.rs"]
mod fixture;
use conduit_core::revision::{RevisionDeltaRole, RevisionDomain};
use conduit_language::{lexical::*, parser_revision::PreparedParserRevisionRebase, revision::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{language_revision::*, semantic::*};
use fixture::*;

#[test]
fn synthetic_asr_revisions_feed_common_language_plan_play_without_asr_only_semantics() {
    let stream = ListeningStreamId::new("synthetic/stream".into()).unwrap();
    let segment = ListeningSegmentId::new("segment".into()).unwrap();
    let text = LanguageTextId::new("asr/text".into()).unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let basis = AsrLanguageBasis {
        stream: &stream,
        segment: &segment,
        text: &text,
        language: &language,
    };
    let initial = partial(10, "you wou");
    let PreparedAsrChange::Revision(first) = prepare_asr_revision(
        &basis,
        &initial,
        None,
        revision("asr/r0"),
        provenance(),
        None,
        0,
        64,
    )
    .unwrap() else {
        panic!("revision")
    };
    let first_bytes = first.revision().clone().encode().unwrap();
    assert!(core::ptr::eq(first.envelope(), &initial));
    let lexical0 = prepare_lexical_tape(first.revision(), &profile(), None).unwrap();
    assert_eq!(lexical0.tape().tokens().len(), 2);
    assert_eq!(
        lexical0.tape().tokens()[1].completeness(),
        &LanguageLexicalCompleteness::TrailingPartial
    );
    assert!(LanguageParserAvailableLexical::new(lexical0.tape().clone(), 2).is_err());
    let available0: LanguageParserRawAvailability = play(
        "language-parser-availability",
        LanguageParserAvailableLexical::new(lexical0.tape().clone(), 1).unwrap(),
    );
    assert!(!*available0.final_input());
    assert_eq!(available0.lexical().tape().source(), first.revision());
    let replacement = revised(12, 4, 7, "would");
    // Supplied synthetic provider snapshot, not stability inferred from a space.
    // This exact snapshot advertises the common seven-scalar prefix explicitly.
    let snapshot = AsrTranscriptCandidate::new(
        segment.clone(),
        None,
        7,
        "you wou".into(),
        1,
        Some("you".into()),
        "you would".into(),
        "ld".into(),
    )
    .unwrap();
    let stable = stable_scalar_prefix(&basis, "you would", &snapshot).unwrap();
    assert_eq!(stable, 7);
    assert_eq!(first.revision().stable_prefix(), &None);
    let PreparedAsrChange::Revision(next) = prepare_asr_revision(
        &basis,
        &replacement,
        Some(&first),
        revision("asr/r1"),
        provenance(),
        Some(stable),
        0,
        64,
    )
    .unwrap() else {
        panic!("revision")
    };
    assert_eq!(next.revision().material().text(), "you would");
    assert_eq!(
        next.revision().prior().as_ref().unwrap().revision(),
        first.revision().material().revision()
    );
    let lexical1 = prepare_lexical_tape(next.revision(), &profile(), Some(&lexical0)).unwrap();
    let unchanged = &lexical1.tape().tokens()[0];
    assert_eq!(unchanged.identity().text_identity(), &text);
    assert_eq!(
        unchanged.identity().text_revision(),
        next.revision().material().revision()
    );
    assert_eq!(
        unchanged.prior_occurrence(),
        &None,
        "the first ASR snapshot admitted no stable occurrence lineage"
    );
    assert_ne!(unchanged.identity(), lexical0.tape().tokens()[0].identity());
    assert_eq!(lexical1.tape().tokens()[1].prior_occurrence(), &None);
    assert!(LanguageParserAvailableLexical::new(lexical1.tape().clone(), 2).is_err());
    let parser_basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/r1".into()).unwrap(),
        next.revision().material().revision().clone(),
        text.clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(
        parser_basis.clone(),
        relation(LanguageUniversalDependencyRelation::Dep),
        1,
    )
    .unwrap();
    let raw: LanguageParserNumericState = play("language-parser-initialize", begin);
    assert_eq!(*raw.committed(), 0);
    let parsed: LanguageParserResult = play(
        "language-parser-transition",
        LanguageParserRequest::new(
            LanguageParserAction::RightArc,
            parser_basis.clone(),
            relation(LanguageUniversalDependencyRelation::Root),
            raw,
        )
        .unwrap(),
    );
    assert!(*parsed.accepted());
    let state = native(parsed.state());
    let common_commit: LanguageParserBeamCommitResult = play(
        "language-parser-beam-commit",
        commit_request(&state, false, false),
    );
    assert_eq!(
        common_commit.refusal(),
        &LanguageParserFrontierRefusal::None
    );
    assert_eq!(*common_commit.frontier().committed(), 1);
    let previous = native(common_commit.candidate0().state());
    let protected_end = u32::try_from(*lexical1.tape().tokens()[0].span().end()).unwrap();
    // Consumer protection comes from an actual Source-committed occurrence,
    // independently of ASR partial/final metadata. It is not a played frontier.
    assert_eq!(protected_end, 3);
    let final_event = committed(15, "you would");
    let PreparedAsrChange::Revision(final_revision) = prepare_asr_revision(
        &basis,
        &final_event,
        Some(&next),
        revision("asr/r2"),
        provenance(),
        Some(9),
        protected_end,
        64,
    )
    .unwrap() else {
        panic!("revision")
    };
    assert_eq!(
        final_revision.revision().finality(),
        &LanguageTextFinality::Final
    );
    let rebase = PreparedParserRevisionRebase::prepare(
        &previous,
        &lexical1,
        final_revision.revision(),
        LanguageAnalysisRevisionId::new("analysis/r2".into()).unwrap(),
        relation(LanguageUniversalDependencyRelation::Dep),
    )
    .unwrap();
    let availability: LanguageParserRawAvailability = play(
        "language-parser-availability",
        rebase.request().next().clone(),
    );
    assert!(*availability.final_input());
    assert_eq!(
        availability.lexical().tape().source(),
        final_revision.revision()
    );
    let rebased: LanguageParserNumericState =
        play("language-parser-revision-rebase", rebase.request().clone());
    assert_eq!(
        *rebased.committed(),
        1,
        "ASR finality must not syntactically commit token two"
    );
    assert_eq!(rebased.heads()[0], previous.heads()[0]);
    assert_eq!(rebased.relation0(), previous.relation0());
    let parsed: LanguageParserResult = play(
        "language-parser-transition",
        LanguageParserRequest::new(
            LanguageParserAction::RightArc,
            rebased.basis().clone(),
            relation(LanguageUniversalDependencyRelation::Dep),
            rebased,
        )
        .unwrap(),
    );
    assert!(*parsed.accepted());
    let state = native(parsed.state());
    let state_before = state.clone().encode().unwrap();
    for (cancelled, pressure, reason) in [
        (true, false, LanguageParserFrontierRefusal::Cancelled),
        (false, true, LanguageParserFrontierRefusal::Pressure),
    ] {
        let request = commit_request(&state, cancelled, pressure);
        let unchanged_frontier = request.frontier().clone();
        let refused: LanguageParserBeamCommitResult = play("language-parser-beam-commit", request);
        assert!(!*refused.accepted());
        assert_eq!(refused.refusal(), &reason);
        assert_eq!(refused.frontier(), &unchanged_frontier);
        assert_eq!(
            native(refused.candidate0().state())
                .clone()
                .encode()
                .unwrap(),
            state_before
        );
    }
    let final_commit: LanguageParserBeamCommitResult = play(
        "language-parser-beam-commit",
        commit_request(&state, false, false),
    );
    assert_eq!(final_commit.refusal(), &LanguageParserFrontierRefusal::None);
    assert_eq!(*final_commit.frontier().committed(), 2);
    // Existing dependency-analysis consumer sees exactly the ASR-originated epoch.
    // The root arc is supplied from the reviewed Source transition, not learned.
    let dependent = LanguageAnalysisTokenRef::new(
        state.basis().analysis_revision().clone(),
        rebase.lexical().tape().tokens()[0].identity().clone(),
    )
    .unwrap();
    let arc = LanguageDependencyArc::new(
        dependent,
        LanguageDependencyHead::Root,
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Root, None).unwrap(),
    )
    .unwrap();
    let domain = DependencyRevisions {
        source_text: final_revision.revision().material(),
        tokens: 2,
    };
    let delta = DependencyDelta::Assert(arc);
    assert!(domain.validate_delta(RevisionDeltaRole::Proposal, &delta));
    let old_domain = DependencyRevisions {
        source_text: first.revision().material(),
        tokens: 2,
    };
    assert!(!old_domain.validate_delta(RevisionDeltaRole::Proposal, &delta));
    // Late/stale/conflicting recognition events cannot rewrite retained facts.
    let conflict = revised(16, 0, 3, "not");
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &conflict,
            Some(&final_revision),
            revision("asr/conflict"),
            provenance(),
            Some(9),
            protected_end,
            64
        ),
        Err(AsrLanguageRefusal::Revision(
            TextRevisionRefusal::CommittedPrefixChanged
        ))
    ));
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &replacement,
            Some(&final_revision),
            revision("asr/late"),
            provenance(),
            Some(9),
            protected_end,
            64
        ),
        Err(AsrLanguageRefusal::Sequence)
    ));
    assert!(matches!(
        prepare_asr_revision(
            &basis,
            &final_event,
            Some(&next),
            revision("asr/r1"),
            provenance(),
            Some(9),
            protected_end,
            64
        ),
        Err(AsrLanguageRefusal::Revision(
            TextRevisionRefusal::PriorRevision
        ))
    ));
    let final_bytes = final_revision.revision().clone().encode().unwrap();
    let cancelled_event = cancelled(17);
    let PreparedAsrChange::Cancelled {
        envelope,
        previous: retained,
    } = prepare_asr_revision(
        &basis,
        &cancelled_event,
        Some(&final_revision),
        revision("unused/cancel"),
        provenance(),
        None,
        protected_end,
        64,
    )
    .unwrap()
    else {
        panic!("cancellation")
    };
    assert!(core::ptr::eq(envelope, &cancelled_event));
    assert!(core::ptr::eq(retained, &final_revision));
    assert_eq!(first.revision().clone().encode().unwrap(), first_bytes);
    assert_eq!(state.clone().encode().unwrap(), state_before);
    assert_eq!(
        final_revision.revision().clone().encode().unwrap(),
        final_bytes
    );
}

//! Correctly classified synthetic ASR snapshots, not model recognition evidence.
//! The consumer is the existing Language lexical/parser/analysis spine. ASR
//! finality never supplies syntactic commitment or physical playback authority.
#[path = "../../../semantics/speech/tests/common/asr_common_language.rs"]
mod fixture;
use conduit_core::revision::{RevisionDeltaRole, RevisionDomain};
use conduit_core::{BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection};
use conduit_language::{lexical::*, revision::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_speech::{language_revision::*, semantic::*};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use fixture::*;
use std::{collections::BTreeMap, time::Duration};

#[test]
fn installed_synthetic_asr_revisions_feed_common_language_without_parallel_semantics() {
    // Share only checked Source, never runtime state or an accepted outcome.
    let parsed = parse_syntax_document(&source());
    let checked = check_syntax_document(&parsed, &StartupCatalog::new()).unwrap();

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
        &checked,
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
    let raw: LanguageParserNumericState = play(&checked, "language-parser-initialize", begin);
    assert_eq!(*raw.committed(), 0);
    let parsed: LanguageParserResult = play(
        &checked,
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
        &checked,
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
        &checked,
        "language-parser-availability",
        rebase.request().next().clone(),
    );
    assert!(*availability.final_input());
    assert_eq!(
        availability.lexical().tape().source(),
        final_revision.revision()
    );
    let rebased: LanguageParserNumericState = play(
        &checked,
        "language-parser-revision-rebase",
        rebase.request().clone(),
    );
    assert_eq!(
        *rebased.committed(),
        1,
        "ASR finality must not syntactically commit token two"
    );
    assert_eq!(rebased.heads()[0], previous.heads()[0]);
    assert_eq!(rebased.relation0(), previous.relation0());
    let parsed: LanguageParserResult = play(
        &checked,
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
        let refused: LanguageParserBeamCommitResult =
            play(&checked, "language-parser-beam-commit", request);
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
        &checked,
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

struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("synthetic ASR Language consumer has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<conduit_std_host::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare(
    checked: &conduit_plot::CheckedSyntaxDocument,
    entry: &str,
    input_port: &str,
    output_port: &str,
) -> (StdHost, conduit_core::Plan) {
    let authored =
        expand_canonical_plot_for_authoring(checked, entry, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        if !advertisement
            .capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, input_port),
        (PortDirection::Output, output_port),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
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
            connection_byte_capacity: 65536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    (host, plan)
}
fn run(
    host: &mut StdHost,
    plan: &conduit_core::Plan,
    port: &str,
    inputs: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let count = inputs.len();
    let inputs = inputs
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: port.into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .collect::<Vec<_>>();
    let mut output = Outputs(Vec::with_capacity(count));
    for input in &inputs {
        // Each authored Item advances one finite ordinary Play. Caller batching
        // and fresh-host partitioning do not introduce a second runtime loop.
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                std::slice::from_ref(input),
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        let kernel = report.kernel.unwrap();
        println!(
            "asr-common-language-source={:?} plan={:?} play={:?}",
            plan.source_document_id, plan.plan_id, kernel.active_play_id
        );
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
    assert_eq!(output.0.len(), count);
    output.0.into_iter().map(|bytes| bytes.to_vec()).collect()
}

/// Admission and all transition/commit policy remain the common Language Source.
fn play<I: NativeRustBinding, O: NativeRustBinding>(
    checked: &conduit_plot::CheckedSyntaxDocument,
    entry: &str,
    input: I,
) -> O {
    let (input_port, output_port) = if entry == "language-parser-transition" {
        ("request", "result")
    } else {
        ("input", "output")
    };
    let (mut host, plan) = prepare(checked, entry, input_port, output_port);
    assert_eq!(plan.source_document_id, checked.source_document_id);
    let outputs = run(&mut host, &plan, input_port, vec![input.encode().unwrap()]);
    O::decode(&outputs[0]).unwrap()
}

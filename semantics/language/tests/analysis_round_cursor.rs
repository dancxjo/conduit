//! Shared complete-round cursor prerequisite; not installed streaming acceptance.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "8",
    target_has_atomic = "ptr"
))]
#[path = "common/parser_round_model_channel.rs"]
mod numeric;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{
        fallback_round::{cursor::*, *},
        *,
    },
    lexical::prepare_lexical_tape,
    revision::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
fn no_growth<T>(operation: impl FnOnce() -> T) -> T {
    let (result, allocations) = allocation_probe::observe(operation);
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    result
}
#[test]
fn complete_cursor_preserves_lexical_survivors_and_replays_each_source_request_once_without_growth()
{
    let mut runtime = runtime::Runtime::new();
    assert_eq!(
        runtime.source_id.as_str(),
        parser_commitment::COMMITMENT_SOURCE_ID
    );
    let source = fixture::revision("answer ", None, 7);
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 1,
    };
    let lexical = prepare_lexical_tape(&source, &fixture::profile(), None).unwrap();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/cursor".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis, fixture::dep(), 1).unwrap();
    let actual = LanguageParserNumericState::decode(&runtime.execute(
        "language-parser-initialize",
        &begin.clone().encode().unwrap(),
    ))
    .unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &begin, &actual)
        .unwrap();
    let owner = AnalysisEpochOwner::initialize(
        initial,
        &domain,
        RevisionContext {
            stream: text("analysis"),
            subject: text("utterance"),
            epoch: text("cursor/1"),
            producer: text("actual-source-fixture"),
            policy: text("zero-score-fallback@1"),
        },
        RevisionLimits {
            history_events: 16,
            revisable_units: 4,
        },
    )
    .unwrap();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let before = owner.beam().clone().encode().unwrap();
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(&owner, programs.checked_document(), &programs)
            .unwrap();
    let mut dispatched = 0;
    while cursor.progress() != CursorProgress::Complete {
        if cursor.progress() == CursorProgress::ReadySource {
            let lease = no_growth(|| cursor.issue_source().unwrap());
            let request = cursor.source_request(&lease).unwrap();
            assert_eq!(request.ordinal, dispatched);
            let output = runtime.execute(request.operation, request.input);
            no_growth(|| {
                cursor
                    .submit_source(lease, &output)
                    .map_err(|v| v.refusal)
                    .unwrap()
            });
            dispatched += 1;
        } else {
            no_growth(|| cursor.step().unwrap());
        }
    }
    assert_eq!(cursor.observation_count(), dispatched);
    assert!(dispatched > 20);
    assert_eq!((0..dispatched).filter(|i|cursor.observation(*i).unwrap().operation=="language-parser-scorer-branch").count(),4);
    assert!((0..dispatched).all(|i| cursor.observation(i).unwrap().admitted));
    let result = LanguageParserScorerBeam::decode(cursor.result_canonical().unwrap()).unwrap();
    let slots = [
        result.candidate0(),
        result.candidate1(),
        result.candidate2(),
        result.candidate3(),
    ];
    assert!(slots.iter().all(|slot| *slot.parser().active()));
    assert!(slots.iter().any(|slot| slot.choices()[0] == 0));
    assert!(slots.iter().any(|slot| slot.choices()[0] == 1));
    assert!(slots
        .iter()
        .all(|slot| *slot.parser().state().committed() == 0));
    assert_eq!(owner.beam().clone().encode().unwrap(), before);
    assert_eq!(owner.generation(), 0);
    // The hosted wrapper drives the identical cursor and admits this retained
    // actual trace; it does not retain the former recursive traversal.
    let observations = (0..dispatched)
        .map(|i| {
            let v = cursor.observation(i).unwrap();
            FallbackOperationObservation {
                operation: v.operation.into(),
                input: v.input.into(),
                output: v.output.into(),
            }
        })
        .collect();
    let hosted = owner
        .prepare_fallback_round(&programs, observations)
        .map_err(|v| v.refusal)
        .unwrap();
    assert_eq!(hosted.result(), &result);
    assert_eq!(hosted.observations().len(), dispatched);
    assert_eq!(hosted.mode(), AnalysisRoundMode::ZeroScoreFallback);
    refusals(&owner, &programs, &mut runtime);
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "8",
        target_has_atomic = "ptr"
    ))]
    learned_cursor(&owner, &programs, &mut runtime);
    runtime.assert_signs();
}
fn ready(cursor: &mut PreparedRoundCursor<'_>) {
    while cursor.progress() != CursorProgress::ReadySource {
        no_growth(|| cursor.step().unwrap());
    }
}
fn refusals(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    runtime: &mut runtime::Runtime,
) {
    let document = programs.checked_document();
    let mut a = PreparedRoundCursor::prepare_fallback(owner, document, programs).unwrap();
    let mut b = PreparedRoundCursor::prepare_fallback(owner, document, programs).unwrap();
    let handle = no_growth(|| a.instance_handle());
    assert!(handle.matches(&a));
    assert!(!handle.matches(&b));
    ready(&mut a);
    ready(&mut b);
    let lease = no_growth(|| a.issue_source().unwrap());
    let foreign = no_growth(|| b.issue_source().unwrap());
    let request = a.source_request(&lease).unwrap();
    let output = runtime.execute(request.operation, request.input);
    let failure = no_growth(|| a.submit_source(foreign, &output).err().unwrap());
    assert_eq!(failure.refusal, CursorRefusal::ForeignLease);
    assert!(b.source_request(&failure.lease).is_ok());
    let oversized = vec![0; 65537];
    let pressure = no_growth(|| a.submit_source(lease, &oversized).err().unwrap());
    assert_eq!(pressure.refusal, CursorRefusal::Pressure);
    assert!(a.source_request(&pressure.lease).is_ok());
    let mut altered = output.clone();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    no_growth(|| {
        a.submit_source(pressure.lease, &altered)
            .map_err(|v| v.refusal)
            .unwrap()
    });
    let reason = loop {
        match no_growth(|| a.step()) {
            Ok(_) => (),
            Err(reason) => break reason,
        }
    };
    assert_eq!(reason, CursorRefusal::ForeignOutput);
    assert_eq!(a.observation(0).unwrap().output, altered);
    assert_eq!(a.expected_source_output().unwrap(), output);
    assert!(!a.observation(0).unwrap().admitted);
    assert!(a.result_canonical().is_err());
    no_growth(|| b.cancel());
    assert!(b.source_request(&failure.lease).is_err());
    assert_eq!(b.step(), Err(CursorRefusal::Cancelled));
    assert!(b.result_canonical().is_err());
    assert_eq!(owner.generation(), 0);
}

#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "8",
    target_has_atomic = "ptr"
))]
fn learned_cursor(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    runtime: &mut runtime::Runtime,
) {
    use conduit_language::analysis_epoch_owner::fallback_round::learned::*;
    let profile = numeric::profile();
    let (document, catalog, definition) = numeric::definition(&profile);
    let mut channel = PreparedLearnedRoundChannel::prepare_canonical(
        &document,
        "parser-round-numeric",
        &catalog,
        definition.clone(),
        profile.clone(),
        ParserRoundModel::AcceptedEnglishV2,
        16,
    )
    .unwrap();
    let mut foreign = PreparedLearnedRoundChannel::prepare_canonical(
        &document,
        "parser-round-numeric",
        &catalog,
        definition,
        profile,
        ParserRoundModel::AcceptedEnglishV2,
        16,
    )
    .unwrap();
    let mut cursor = PreparedRoundCursor::prepare_learned(
        owner,
        programs.checked_document(),
        programs,
        &mut channel,
    )
    .unwrap();
    let mut checked_foreign = false;
    while cursor.progress() != CursorProgress::Complete {
        match cursor.progress() {
            CursorProgress::ReadySource => {
                let lease = no_growth(|| cursor.issue_source().unwrap());
                let request = cursor.source_request(&lease).unwrap();
                let output = runtime.execute(request.operation, request.input);
                no_growth(|| {
                    cursor
                        .submit_source(lease, &output)
                        .map_err(|v| v.refusal)
                        .unwrap()
                });
            }
            CursorProgress::ReadyNumerical => {
                if !checked_foreign {
                    assert_eq!(
                        no_growth(|| foreign.step(&mut cursor)),
                        Err(CursorRefusal::ForeignLease)
                    );
                    assert!(!foreign.is_stopped());
                    assert!(!channel.is_stopped());
                    assert_eq!(cursor.progress(), CursorProgress::ReadyNumerical);
                    checked_foreign = true;
                }
                no_growth(|| channel.step(&mut cursor).unwrap());
            }
            _ => {
                no_growth(|| cursor.step().unwrap());
            }
        }
    }
    assert!(checked_foreign);
    assert!(!cursor.numerical_evidence().is_empty());
    for (ordinal, evidence) in cursor.numerical_evidence().iter().enumerate() {
        let lease = evidence.request();
        assert_eq!(lease.request_ordinal(), ordinal);
        assert_eq!(lease.generation(), owner.generation());
        let source = cursor.observation(lease.source_ordinal()).unwrap();
        assert_eq!(source.operation, "language-parser-scorer-model-features");
        assert!(source.admitted);
        let query = LanguageParserScorerChoiceQuery::decode(source.input).unwrap();
        let features = LanguageParserScorerFeatures::decode(source.output).unwrap();
        assert_eq!(features.invocation(), query.invocation());
        assert_eq!(evidence.numerical().invocation(), *query.invocation());
        assert_eq!(lease.invocation(), *query.invocation());
        assert!(!evidence.numerical().active_plays().is_empty());
        assert!(conduit_core::verify_plan(
            &evidence.definition().internal_plan
        ));
        assert_eq!(
            evidence.profile().resource().artifact(),
            &conduit_language::parser_model::english_parser_scorer_descriptor().0
        );
    }
    assert!(channel.pending().is_none());
    assert_eq!(owner.generation(), 0);
    assert_eq!(cursor.mode(), AnalysisRoundMode::AuthenticatedCategorical);
}

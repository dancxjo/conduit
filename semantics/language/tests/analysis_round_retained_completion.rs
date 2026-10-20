//! One actual Source round, retained completion and guarded owner publication.
//! No installed Std/learned/Current-consumer claim: Runtime is existing test host.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{
        fallback_round::{
            cursor::{installed_completion::*, *},
            *,
        },
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
#[test]
fn same_actual_cursor_decodes_detaches_and_applies_with_no_allocator_activity() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    let source = fixture::revision("answer ", None, 7);
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 1,
    };
    let begin = LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/retained-completion".into()).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap(),
        fixture::dep(),
        1,
    )
    .unwrap();
    let mut runtime = runtime::Runtime::new();
    let actual = LanguageParserNumericState::decode(&runtime.execute(
        "language-parser-initialize",
        &begin.clone().encode().unwrap(),
    ))
    .unwrap();
    let initializer = PreparedAnalysisInitializationProgram::prepare().unwrap();
    let initialize = || {
        let lexical = prepare_lexical_tape(&source, &fixture::profile(), None).unwrap();
        let initial = initializer.replay(lexical, &begin, &actual).unwrap();
        AnalysisEpochOwner::initialize(
            initial,
            &domain,
            RevisionContext {
                stream: text("analysis"),
                subject: text("utterance"),
                epoch: text("retained/1"),
                producer: text("actual-source-fixture"),
                policy: text("zero-score-fallback@1"),
            },
            RevisionLimits {
                history_events: 1,
                revisable_units: 4,
            },
        )
        .unwrap()
    };
    let mut owner = initialize();
    let mut foreign = initialize();
    assert_eq!(owner.beam(), foreign.beam());
    assert_eq!(owner.lexical().tape(), foreign.lexical().tape());
    let original = owner.beam().clone().encode().unwrap();
    let original_address = owner.beam() as *const LanguageParserScorerBeam;
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    assert_eq!(
        runtime.source_id,
        programs.checked_document().source_document_id
    );
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(&owner, programs.checked_document(), &programs)
            .unwrap();
    // All completion storage is reserved before this round's actual child Plays.
    let mut completion =
        PreparedInstalledRoundCompletion::prepare(programs.checked_document()).unwrap();
    assert_eq!(completion.progress(), InstalledCompletionProgress::Prepared);
    let blocked_completion =
        PreparedInstalledRoundCompletion::prepare(programs.checked_document()).unwrap();
    let mut dispatched = 0;
    while cursor.progress() != CursorProgress::Complete {
        if cursor.progress() == CursorProgress::ReadySource {
            let lease = cursor.issue_source().unwrap();
            let request = cursor.source_request(&lease).unwrap();
            assert_eq!(request.ordinal, dispatched);
            let output = runtime.execute(request.operation, request.input);
            cursor
                .submit_source(lease, &output)
                .map_err(|failure| failure.refusal)
                .unwrap();
            dispatched += 1;
        } else {
            cursor.step().unwrap();
        }
    }
    let expected = cursor.result_canonical().unwrap().to_vec();
    let expected_native = LanguageParserScorerBeam::decode(&expected).unwrap();
    let expected_prior = cursor.prior_canonical().to_vec();
    let expected_lexical = cursor.lexical_canonical().to_vec();
    let expected_next_identity = cursor.next_identity();
    let frames: Vec<_> = (0..cursor.observation_count())
        .map(|i| {
            let frame = cursor.observation(i).unwrap();
            (
                frame.operation,
                frame.input.to_vec(),
                frame.output.to_vec(),
                frame.admitted,
            )
        })
        .collect();
    assert_eq!(frames.len(), dispatched);
    assert!(!frames.is_empty());
    let source_operations = runtime.operations;
    // Foreign refusal, decode, detach and apply share one measurement. All
    // buffers/tokens/slots survive by returning spent and retaining completion.
    let ((spent, completion_steps), growth) = allocation_probe::observe(|| {
        let mut completion_steps = 0;
        while completion.progress() != InstalledCompletionProgress::Complete {
            completion.step(&cursor).unwrap();
            completion_steps += 1;
        }
        assert_eq!(completion.result().unwrap(), &expected_native);
        assert_eq!(
            cursor
                .completed_result_admission()
                .unwrap()
                .retained_bytes(),
            expected
        );
        let refusal = cursor
            .into_retained_prepared_round(&foreign, &completion)
            .err()
            .unwrap();
        assert!(matches!(refusal.refusal, FallbackRoundRefusal::Stale));
        assert_eq!(refusal.cursor.progress(), CursorProgress::Complete);
        assert_eq!(refusal.cursor.result_canonical().unwrap(), expected);
        assert_eq!(refusal.cursor.observation_count(), frames.len());
        assert_eq!(completion.progress(), InstalledCompletionProgress::Complete);
        assert_eq!(foreign.generation(), 0);
        assert!(foreign.fallback_rounds().is_empty());
        let (prepared, spent) = refusal
            .cursor
            .into_retained_prepared_round(&owner, &completion)
            .map_err(|failure| failure.refusal)
            .unwrap();
        assert_eq!(prepared.mode(), AnalysisRoundMode::ZeroScoreFallback);
        assert_eq!(prepared.result(), &expected_native);
        assert!(core::ptr::eq(
            prepared.result(),
            completion.result().unwrap()
        ));
        assert_eq!(prepared.observations().len(), frames.len());
        let retained = prepared.retained_evidence().unwrap();
        assert!(retained.matches_owner_instance(&owner));
        assert!(!retained.matches_owner_instance(&foreign));
        assert_eq!(retained.result_canonical(), expected);
        assert_eq!(retained.prior_canonical(), expected_prior);
        assert_eq!(retained.lexical_canonical(), expected_lexical);
        assert_eq!(retained.generation(), 0);
        assert_eq!(retained.next_identity(), expected_next_identity);
        assert_eq!(retained.mode(), AnalysisRoundMode::ZeroScoreFallback);
        for (i, expected) in frames.iter().enumerate() {
            let view = prepared.observations().get(i).unwrap();
            let exact = retained.observation(i).unwrap();
            assert_eq!(view.operation, expected.0);
            assert_eq!(view.input, expected.1);
            assert_eq!(view.output, expected.2);
            assert_eq!(view.native_admitted, Some(expected.3));
            assert_eq!(exact.operation, expected.0);
            assert_eq!(exact.input, expected.1);
            assert_eq!(exact.output, expected.2);
            assert_eq!(exact.admitted, expected.3);
        }
        let refused_apply = foreign.apply_fallback_round(prepared).err().unwrap();
        assert!(matches!(refused_apply.refusal, FallbackRoundRefusal::Stale));
        assert_eq!(foreign.generation(), 0);
        assert!(foreign.fallback_rounds().is_empty());
        owner
            .apply_fallback_round(refused_apply.prepared)
            .map_err(|failure| failure.refusal)
            .unwrap();
        let stored = &owner.fallback_rounds()[0];
        assert!(core::ptr::eq(stored.prior(), original_address));
        assert!(core::ptr::eq(stored.result(), owner.beam()));
        assert!(core::ptr::eq(owner.beam(), completion.result().unwrap()));
        (spent, completion_steps)
    });
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert_eq!(growth.deallocations, 0);
    assert!(completion_steps > 0);
    assert_eq!(owner.generation(), 1);
    assert_eq!(owner.beam(), &expected_native);
    assert_eq!(owner.fallback_rounds().len(), 1);
    assert_eq!(
        owner.fallback_rounds()[0].observations().len(),
        frames.len()
    );
    assert_eq!(
        owner.fallback_rounds()[0].prior().clone().encode().unwrap(),
        original
    );
    assert_eq!(foreign.beam().clone().encode().unwrap(), original);
    assert_eq!(
        runtime.operations, source_operations,
        "decode/detach/apply must not execute Source again"
    );
    runtime.assert_signs();
    // The held receipt fills the one reserved owner-history slot. Admission
    // refuses a current-generation, still-unplayed cursor before consuming it.
    let pending =
        PreparedRoundCursor::prepare_fallback(&owner, programs.checked_document(), &programs)
            .unwrap();
    let generation = owner.generation();
    let (blocked, pressure_growth) = allocation_probe::observe(|| {
        pending
            .into_retained_prepared_round(&owner, &blocked_completion)
            .err()
            .unwrap()
    });
    assert!(matches!(blocked.refusal, FallbackRoundRefusal::Capacity));
    assert_eq!(blocked.cursor.observation_count(), 0);
    assert_eq!(
        blocked_completion.progress(),
        InstalledCompletionProgress::Prepared
    );
    assert_eq!(owner.generation(), generation);
    assert_eq!(owner.fallback_rounds().len(), 1);
    assert_eq!(pressure_growth.allocations, 0);
    assert_eq!(pressure_growth.reallocations, 0);
    assert_eq!(pressure_growth.deallocations, 0);
    // Explicitly keep both spent preparation and completion allocations until
    // outside Play/probe; dropping them is retirement, not a hidden publication.
    drop(spent);
    drop(completion);
}

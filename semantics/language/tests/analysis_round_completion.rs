//! Actual Source traversal once, followed by explicitly allocating post-Play conversion.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
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
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
#[test]
fn completed_cursor_converts_without_source_replay_and_refuses_equivalent_foreign_owner() {
    let source = fixture::revision("answer ", None, 7);
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 1,
    };
    let begin = LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/completion".into()).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap(),
        fixture::dep(),
        1,
    )
    .unwrap();
    let mut runtime = runtime::Runtime::new();
    eprintln!("completion Source: {}", runtime.source_id.as_str());
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
                epoch: text("completion/1"),
                producer: text("actual-source-fixture"),
                policy: text("zero-score-fallback@1"),
            },
            RevisionLimits {
                history_events: 16,
                revisable_units: 4,
            },
        )
        .unwrap()
    };
    let mut owner = initialize();
    let mut foreign = initialize();
    let owner_handle = owner.instance_handle();
    assert!(owner_handle.matches(&owner));
    assert!(!owner_handle.matches(&foreign));
    assert_eq!(owner.beam(), foreign.beam());
    assert_eq!(owner.generation(), foreign.generation());
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(&owner, programs.checked_document(), &programs)
            .unwrap();
    let premature = cursor.into_hosted_prepared_round(&owner).err().unwrap();
    assert!(matches!(premature.refusal, FallbackRoundRefusal::Admission));
    cursor = premature.cursor;
    while cursor.progress() != CursorProgress::Complete {
        if cursor.progress() == CursorProgress::ReadySource {
            let lease = cursor.issue_source().unwrap();
            let request = cursor.source_request(&lease).unwrap();
            let actual = runtime.execute(request.operation, request.input);
            cursor
                .submit_source(lease, &actual)
                .map_err(|error| error.refusal)
                .unwrap();
        } else {
            cursor.step().unwrap();
        }
    }
    let expected = cursor.result_canonical().unwrap().to_vec();
    let count = cursor.observation_count();
    let operations = runtime.operations;
    let refusal = cursor.into_hosted_prepared_round(&foreign).err().unwrap();
    assert!(matches!(refusal.refusal, FallbackRoundRefusal::Stale));
    assert_eq!(refusal.cursor.progress(), CursorProgress::Complete);
    assert_eq!(refusal.cursor.result_canonical().unwrap(), expected);
    assert_eq!(refusal.cursor.observation_count(), count);
    let prepared = refusal
        .cursor
        .into_hosted_prepared_round(&owner)
        .map_err(|error| error.refusal)
        .unwrap();
    assert_eq!(
        runtime.operations, operations,
        "post-Play conversion must not execute Source again"
    );
    assert_eq!(prepared.observations().len(), count);
    assert_eq!(prepared.result().clone().encode().unwrap(), expected);
    let refused_apply = foreign.apply_fallback_round(prepared).err().unwrap();
    assert!(matches!(refused_apply.refusal, FallbackRoundRefusal::Stale));
    assert_eq!(foreign.generation(), 0);
    assert!(foreign.fallback_rounds().is_empty());
    let prior_address = owner.beam() as *const LanguageParserScorerBeam;
    let result_address = refused_apply.prepared.result() as *const LanguageParserScorerBeam;
    let (applied, growth) =
        allocation_probe::observe(|| owner.apply_fallback_round(refused_apply.prepared));
    applied.map_err(|error| error.refusal).unwrap();
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert!(core::ptr::eq(owner.beam(), result_address));
    assert!(core::ptr::eq(
        owner.fallback_rounds()[0].prior(),
        prior_address
    ));
    assert!(core::ptr::eq(
        owner.fallback_rounds()[0].result(),
        owner.beam()
    ));
    assert!(owner_handle.matches(&owner));
    assert_eq!(owner.generation(), 1);
    assert_eq!(owner.beam().clone().encode().unwrap(), expected);
    assert_eq!(owner.fallback_rounds().len(), 1);
    assert_eq!(owner.fallback_rounds()[0].observations().len(), count);
    assert_eq!(runtime.operations, operations);
    runtime.assert_signs();
}

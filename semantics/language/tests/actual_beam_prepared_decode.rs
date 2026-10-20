//! Real Source Beam -> generated prepared storage prerequisite only.
//! Does not publish an owner update or grant committed Current authority.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/analysis_epoch_runtime.rs"]
mod runtime;
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*,
    analysis_epoch_owner::{fallback_round::*, *},
    lexical::prepare_lexical_tape,
    revision::*,
    *,
};
use conduit_plot::{rust_binding::*, CheckedSyntaxDocument};
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn bounds() -> PreparedDecodeBounds {
    PreparedDecodeBounds {
        canonical_bytes: 65536,
        leaf_steps: LanguageParserScorerBeam::PREPARED_STORAGE_LEAF_STEPS,
        text_bytes: LanguageParserScorerBeam::PREPARED_STORAGE_TEXT_BYTES,
    }
}
fn text(value: &str) -> RevisionText<'_> {
    RevisionText::new(value).unwrap()
}
/// Authored Source owns all state/slot initialization. Rust supplies varied input
/// identifiers and relation values, then uses existing exact Source replay admission.
fn initial(
    runtime: &mut runtime::Runtime,
    suffix: &str,
    relation: LanguageUniversalDependencyRelation,
) -> (PreparedAnalysisInitialization, LanguageTextRevision) {
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            LanguageTextId::new(format!("utterance/{suffix}")).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(format!("source/{suffix}")).unwrap(),
            "answer record ".into(),
        )
        .unwrap(),
        None,
        fixture::provenance(),
        0,
        Some(14),
    )
    .unwrap();
    let lexical = prepare_lexical_tape(&source, &fixture::profile(), None).unwrap();
    let begin = LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new(format!("analysis/{suffix}")).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap(),
        LanguageParserRelation::new(
            relation,
            LanguageParserSubtype::new(format!("stage-{suffix}")).unwrap(),
        )
        .unwrap(),
        2,
    )
    .unwrap();
    let actual = LanguageParserNumericState::decode(&runtime.execute(
        "language-parser-initialize",
        &begin.clone().encode().unwrap(),
    ))
    .unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(lexical, &begin, &actual)
        .unwrap();
    let envelope = runtime.execute(
        "language/analysis-initial-envelope",
        &actual.encode().unwrap(),
    );
    assert_eq!(envelope, initial.envelope_output_canonical());
    (initial, source)
}
fn admission<'d>(document: &'d CheckedSyntaxDocument, bytes: &[u8]) -> PreparedNativeAdmission<'d> {
    let native = document
        .native_types
        .iter()
        .find(|n| n.name == "LanguageParserScorerBeam")
        .unwrap();
    let mut admission = PreparedNativeAdmission::new(
        document,
        &native.identity,
        PreparedNativeBounds {
            canonical_bytes: 65536,
            scopes: 4096,
            invariants: 4096,
        },
    )
    .unwrap();
    admission.begin(bytes).unwrap();
    while admission.step().unwrap() != PreparedNativeProgress::Admitted {}
    admission
}
#[test]
fn actual_four_slot_source_beam_materializes_without_growth_and_refusals_leave_owner_unchanged() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    let mut runtime = runtime::Runtime::new();
    let (prepared_initial, source) = initial(
        &mut runtime,
        "alpha-longer",
        LanguageUniversalDependencyRelation::Dep,
    );
    let expected = prepared_initial.beam().clone();
    let bytes = expected.clone().encode().unwrap();
    let domain = DependencyRevisions {
        source_text: source.material(),
        tokens: 2,
    };
    let owner = AnalysisEpochOwner::initialize(
        prepared_initial,
        &domain,
        RevisionContext {
            stream: text("decode"),
            subject: text("utterance"),
            epoch: text("epoch/1"),
            producer: text("actual-source"),
            policy: text("decode-prerequisite"),
        },
        RevisionLimits {
            history_events: 8,
            revisable_units: 4,
        },
    )
    .unwrap();
    let before = owner.beam().clone().encode().unwrap();
    let generation = owner.generation();
    let frontiers = owner.frontiers();
    let programs = PreparedFallbackRoundPrograms::prepare().unwrap();
    let document = programs.checked_document();
    assert_eq!(runtime.source_id, document.source_document_id);
    let admitted = admission(document, &bytes);
    let witness = admitted.admitted().unwrap();
    let mut storage = LanguageParserScorerBeam::prepare_storage(document, bounds()).unwrap();
    let ((result, steps), growth) = allocation_probe::observe(|| {
        let mut steps = 0;
        loop {
            steps += 1;
            if storage.step(&witness).unwrap() == PreparedDecodeProgress::Complete {
                break;
            }
        }
        let result = match storage.finish(&witness) {
            Ok(value) => value,
            Err(_) => panic!("admitted Beam materialization refused"),
        };
        (result, steps)
    });
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert_eq!(steps, LanguageParserScorerBeam::PREPARED_STORAGE_LEAF_STEPS);
    assert_eq!(result, expected);
    for slot in [
        result.candidate0(),
        result.candidate1(),
        result.candidate2(),
        result.candidate3(),
    ] {
        assert_eq!(
            slot.parser().state().basis().text(),
            source.material().identity()
        );
        assert_eq!(
            slot.parser().state().basis().source_revision(),
            source.material().revision()
        );
        assert_eq!(
            slot.parser().state().relation3().subtype().get().as_str(),
            "stage-alpha-longer"
        );
        assert_eq!(*slot.parser().state().heads(), [5, 5, 5, 5, 4]);
        assert_eq!(*slot.choices(), [0; 4]);
        assert_eq!(*slot.selected(), 0);
    }
    // A second real Source result changes identifiers, subtype and nullary UD tag.
    let (second, _) = initial(
        &mut runtime,
        "beta",
        LanguageUniversalDependencyRelation::Nsubj,
    );
    let second_bytes = second.beam().clone().encode().unwrap();
    assert_ne!(bytes, second_bytes);
    let second_admitted = admission(document, &second_bytes);
    let second_witness = second_admitted.admitted().unwrap();
    let mut second_storage = LanguageParserScorerBeam::prepare_storage(document, bounds()).unwrap();
    loop {
        if second_storage.step(&second_witness).unwrap() == PreparedDecodeProgress::Complete {
            break;
        }
    }
    let second_result = match second_storage.finish(&second_witness) {
        Ok(value) => value,
        Err(_) => panic!("second real Beam refused"),
    };
    assert_eq!(&second_result, second.beam());
    let mut changed = LanguageParserScorerBeam::prepare_storage(document, bounds()).unwrap();
    changed.step(&witness).unwrap();
    assert_eq!(
        changed.step(&second_witness),
        Err(PreparedDecodeRefusal::ForeignWitness)
    );
    assert!(changed.finish(&witness).is_err());
    let foreign_document = document.clone();
    assert_eq!(
        foreign_document.source_document_id,
        document.source_document_id
    );
    let foreign_admitted = admission(&foreign_document, &bytes);
    let foreign_witness = foreign_admitted.admitted().unwrap();
    let mut foreign = LanguageParserScorerBeam::prepare_storage(document, bounds()).unwrap();
    assert_eq!(
        foreign.step(&foreign_witness),
        Err(PreparedDecodeRefusal::ForeignWitness)
    );
    assert!(foreign.finish(&witness).is_err());
    let mut weakened = document.clone();
    let state = weakened
        .native_types
        .iter_mut()
        .find(|n| n.name == "LanguageParserState")
        .unwrap();
    assert!(!state.invariants.is_empty());
    state.invariants.clear();
    assert!(LanguageParserScorerBeam::prepare_storage(&weakened, bounds()).is_err());
    let mut cancelled = LanguageParserScorerBeam::prepare_storage(document, bounds()).unwrap();
    cancelled.step(&witness).unwrap();
    cancelled.cancel();
    assert_eq!(
        cancelled.step(&witness),
        Err(PreparedDecodeRefusal::Cancelled)
    );
    assert!(cancelled.finish(&witness).is_err());
    let mut pressure = LanguageParserScorerBeam::prepare_storage(
        document,
        PreparedDecodeBounds {
            canonical_bytes: 1,
            ..bounds()
        },
    )
    .unwrap();
    assert_eq!(
        pressure.step(&witness),
        Err(PreparedDecodeRefusal::Pressure)
    );
    assert!(pressure.finish(&witness).is_err());
    assert_eq!(owner.beam().clone().encode().unwrap(), before);
    assert_eq!(owner.generation(), generation);
    assert_eq!(owner.frontiers(), frontiers);
    runtime.assert_signs();
}

//! Current four-slot Native materialization with real Source-initialized states.
//! No epoch controller, owner publication or committed Current authority.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[path = "common/parser_selection_io.rs"]
mod parser_selection_io;
use conduit_core::*;
use conduit_language::*;
use conduit_plot::{rust_binding::*, *};

#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn bounds() -> PreparedDecodeBounds {
    PreparedDecodeBounds {
        canonical_bytes: 65536,
        leaf_steps: LanguageParserScorerBeam::PREPARED_STORAGE_LEAF_STEPS,
        text_bytes: LanguageParserScorerBeam::PREPARED_STORAGE_TEXT_BYTES,
    }
}
fn checked_source() -> CheckedSyntaxDocument {
    let source = parse_syntax_document(&parser_selection_io::language_source());
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    check_syntax_document(&source, &startup).unwrap()
}
fn initial(
    graph: &mut parser_selection_io::SourceGraph,
    suffix: &str,
    relation: LanguageUniversalDependencyRelation,
) -> LanguageParserScorerBeam {
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new(format!("analysis/{suffix}")).unwrap(),
        LanguageTextRevisionId::new(format!("source/{suffix}")).unwrap(),
        LanguageTextId::new(format!("utterance/{suffix}")).unwrap(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(
        basis,
        LanguageParserRelation::new(
            relation,
            LanguageParserSubtype::new(format!("stage-{suffix}")).unwrap(),
        )
        .unwrap(),
        2,
    )
    .unwrap();
    let observed = graph.run(begin);
    let numeric = LanguageParserNumericState::from_structured(observed).unwrap();
    // Reuse the existing exact Native state admission and checked constructors,
    // as in prepared_beam_admission; no shape-derived Native decoder is added.
    let state = fixture::native_state(&numeric);
    let inactive = LanguageParserScorerHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(false, 2, 0, state.clone()).unwrap(),
        *state.unread(),
    )
    .unwrap();
    let seed = LanguageParserScorerHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(true, 1, -7, state).unwrap(),
        *inactive.selected(),
    )
    .unwrap();
    LanguageParserScorerBeam::new(seed, inactive.clone(), inactive.clone(), inactive).unwrap()
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
            contracts: 4096,
        },
    )
    .unwrap();
    admission.begin(bytes).unwrap();
    while admission.step().unwrap() != PreparedNativeProgress::Admitted {}
    admission
}
fn no_growth(growth: allocation_probe::Observation) {
    assert_eq!(growth.allocations, 0);
    assert_eq!(growth.reallocations, 0);
    assert_eq!(growth.deallocations, 0);
}
fn materialize<'d>(
    mut storage: LanguageParserScorerBeamPreparedStorage<'d>,
    witness: &AdmittedNativeCanonical<'_>,
) -> (LanguageParserScorerBeam, PreparedDecodeGate<'d>) {
    let ((value, retained, steps), growth) = allocation_probe::observe(|| {
        let mut steps = 0;
        loop {
            steps += 1;
            if storage.step(witness).unwrap() == PreparedDecodeProgress::Complete {
                break;
            }
        }
        let (value, retained) = match storage.finish_retained(witness) {
            Ok(value) => value,
            Err(_) => panic!("admitted Beam materialization refused"),
        };
        (value, retained, steps)
    });
    no_growth(growth);
    assert_eq!(steps, LanguageParserScorerBeam::PREPARED_STORAGE_LEAF_STEPS);
    (value, retained)
}
fn refusal<'d>(
    result: Result<
        (LanguageParserScorerBeam, PreparedDecodeGate<'d>),
        (
            PreparedDecodeRefusal,
            LanguageParserScorerBeamPreparedStorage<'d>,
        ),
    >,
    expected: PreparedDecodeRefusal,
) -> LanguageParserScorerBeamPreparedStorage<'d> {
    match result {
        Err((reason, storage)) => {
            assert_eq!(reason, expected);
            storage
        }
        Ok(_) => panic!("refused private staging published a Native value"),
    }
}
#[test]
fn actual_four_slot_native_beams_materialize_without_growth_and_refusals_preserve_custody() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    let document = checked_source();
    // Finite two-use private pool, reserved before either ordinary Source Play.
    let first_storage = LanguageParserScorerBeam::prepare_storage(&document, bounds()).unwrap();
    let second_storage = LanguageParserScorerBeam::prepare_storage(&document, bounds()).unwrap();
    let mut graph = parser_selection_io::SourceGraph::language("language-parser-initialize");
    let first = initial(
        &mut graph,
        "alpha-longer",
        LanguageUniversalDependencyRelation::Dep,
    );
    let bytes = first.clone().encode().unwrap();
    let admitted = admission(&document, &bytes);
    let witness = admitted.admitted().unwrap();
    let (value, retained) = materialize(first_storage, &witness);
    assert_eq!(value, first);
    assert_eq!(value.clone().encode().unwrap(), bytes);
    assert!(retained.canonical_capacity() >= bytes.len());
    for slot in [
        value.candidate0(),
        value.candidate1(),
        value.candidate2(),
        value.candidate3(),
    ] {
        assert_eq!(
            slot.parser().state().basis().text().get().as_str(),
            "utterance/alpha-longer"
        );
        assert_eq!(
            slot.parser().state().relation3().subtype().get().as_str(),
            "stage-alpha-longer"
        );
        assert_eq!(*slot.parser().state().heads(), [5, 5, 5, 5, 4]);
        assert_eq!(*slot.choices(), [0; 4]);
        assert_eq!(*slot.selected(), 0);
    }
    assert!(*value.candidate0().parser().active());
    assert_eq!(*value.candidate0().parser().score(), -7);
    assert!(!*value.candidate1().parser().active());
    let second = initial(
        &mut graph,
        "beta",
        LanguageUniversalDependencyRelation::Nsubj,
    );
    let second_bytes = second.clone().encode().unwrap();
    assert_ne!(bytes, second_bytes);
    let second_admitted = admission(&document, &second_bytes);
    let second_witness = second_admitted.admitted().unwrap();
    let (second_value, second_retained) = materialize(second_storage, &second_witness);
    assert_eq!(second_value, second);
    assert!(second_retained.canonical_capacity() >= second_bytes.len());

    let mut changed = LanguageParserScorerBeam::prepare_storage(&document, bounds()).unwrap();
    let (changed, growth) = allocation_probe::observe(|| {
        changed.step(&witness).unwrap();
        assert_eq!(
            changed.step(&second_witness),
            Err(PreparedDecodeRefusal::ForeignWitness)
        );
        refusal(
            changed.finish_retained(&witness),
            PreparedDecodeRefusal::ForeignWitness,
        )
    });
    no_growth(growth);
    assert!(changed.canonical_capacity() >= bytes.len());

    let foreign_document = document.clone();
    assert_eq!(
        foreign_document.source_document_id,
        document.source_document_id
    );
    let foreign_admitted = admission(&foreign_document, &bytes);
    let foreign_witness = foreign_admitted.admitted().unwrap();
    let mut foreign = LanguageParserScorerBeam::prepare_storage(&document, bounds()).unwrap();
    let (foreign, growth) = allocation_probe::observe(|| {
        assert_eq!(
            foreign.step(&foreign_witness),
            Err(PreparedDecodeRefusal::ForeignWitness)
        );
        refusal(
            foreign.finish_retained(&witness),
            PreparedDecodeRefusal::ForeignWitness,
        )
    });
    no_growth(growth);
    assert!(foreign.canonical_capacity() >= bytes.len());

    let mut weakened = document.clone();
    let state = weakened
        .native_types
        .iter_mut()
        .find(|n| n.name == "LanguageParserState")
        .unwrap();
    assert!(!state.invariants.is_empty());
    state.invariants.clear();
    assert!(LanguageParserScorerBeam::prepare_storage(&weakened, bounds()).is_err());
    let mut weakened_contract = document.clone();
    let subtype = weakened_contract
        .native_types
        .iter_mut()
        .find(|n| n.name == "LanguageParserSubtype")
        .unwrap();
    assert!(!subtype.value_contracts.is_empty());
    subtype.value_contracts.clear();
    assert!(LanguageParserScorerBeam::prepare_storage(&weakened_contract, bounds()).is_err());

    let mut cancelled = LanguageParserScorerBeam::prepare_storage(&document, bounds()).unwrap();
    let (cancelled, growth) = allocation_probe::observe(|| {
        cancelled.step(&witness).unwrap();
        cancelled.cancel();
        assert_eq!(
            cancelled.step(&witness),
            Err(PreparedDecodeRefusal::Cancelled)
        );
        refusal(
            cancelled.finish_retained(&witness),
            PreparedDecodeRefusal::Cancelled,
        )
    });
    no_growth(growth);
    assert!(cancelled.canonical_capacity() >= bytes.len());
    let mut incomplete = LanguageParserScorerBeam::prepare_storage(&document, bounds()).unwrap();
    let (incomplete, growth) = allocation_probe::observe(|| {
        incomplete.step(&witness).unwrap();
        refusal(
            incomplete.finish_retained(&witness),
            PreparedDecodeRefusal::State,
        )
    });
    no_growth(growth);
    assert!(incomplete.canonical_capacity() >= bytes.len());
    let mut pressure = LanguageParserScorerBeam::prepare_storage(
        &document,
        PreparedDecodeBounds {
            canonical_bytes: 1,
            ..bounds()
        },
    )
    .unwrap();
    let (pressure, growth) = allocation_probe::observe(|| {
        assert_eq!(
            pressure.step(&witness),
            Err(PreparedDecodeRefusal::Pressure)
        );
        refusal(
            pressure.finish_retained(&witness),
            PreparedDecodeRefusal::Pressure,
        )
    });
    no_growth(growth);
    assert_eq!(pressure.canonical_capacity(), 1);
    assert_eq!(first.clone().encode().unwrap(), bytes);
    assert_eq!(second.clone().encode().unwrap(), second_bytes);
    assert_eq!(admitted.retained_bytes(), bytes);
    assert_eq!(second_admitted.retained_bytes(), second_bytes);
}

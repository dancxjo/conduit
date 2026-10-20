//! Native lexical-query closure only; portable initialization is not Play evidence.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
use conduit_core::*;
use conduit_language::{analysis_epoch_initialization::*, lexical::*, *};
use conduit_plot::{rust_binding::*, *};
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn checked() -> CheckedSyntaxDocument {
    let syntax = parse_syntax_document(language_semantic_source());
    assert_eq!(
        syntax.source_document_id().as_str(),
        parser_commitment::COMMITMENT_SOURCE_ID
    );
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    check_syntax_document(&syntax, &startup).unwrap()
}
fn query(tape: PreparedLexicalTape) -> LanguageParserScorerChoiceQuery {
    let material = tape.tape().source().material();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/lexical-closure".into()).unwrap(),
        material.revision().clone(),
        material.identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis, fixture::dep(), 1).unwrap();
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/analysis_initialize_program.hex"
    )))
    .unwrap();
    let observed = LanguageParserNumericState::decode(
        &program.evaluate(&begin.clone().encode().unwrap()).unwrap(),
    )
    .unwrap();
    let lexical = LanguageParserAvailableLexical::new(tape.tape().clone(), 1).unwrap();
    let initial = PreparedAnalysisInitializationProgram::prepare()
        .unwrap()
        .replay(tape, &begin, &observed)
        .unwrap();
    LanguageParserScorerChoiceQuery::new(
        [0; 4],
        *lexical.tape().source().sequence(),
        1,
        lexical,
        initial.beam().candidate0().parser().state().clone(),
    )
    .unwrap()
}
fn field<'a>(
    v: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> ValidatedCanonicalStructuredValue<'a> {
    v.record_field(name).unwrap().unwrap()
}
fn run(
    admission: &mut PreparedNativeAdmission<'_>,
    input: &[u8],
) -> Result<(), PreparedNativeRefusal> {
    admission.begin(input)?;
    loop {
        if admission.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
#[test]
fn full_actual_lexical_query_admission_matches_generated_native_with_none_and_some_history_without_growth(
) {
    let document = checked();
    let native = document
        .native_types
        .iter()
        .find(|v| v.name == "LanguageParserScorerChoiceQuery")
        .unwrap();
    assert_eq!(
        native.value_type,
        LanguageParserScorerChoiceQuery::semantic_type().unwrap()
    );
    let old = fixture::revision("answer ", None, 7);
    let next = fixture::revision("answer ", Some(&old), 7);
    let prior = prepare_lexical_tape(&old, &fixture::profile(), None).unwrap();
    let current = prepare_lexical_tape(&next, &fixture::profile(), Some(&prior)).unwrap();
    let current_query = query(current);
    for query in [query(prior), current_query] {
        let input = query.clone().encode().unwrap();
        let mut admission = PreparedNativeAdmission::new(
            &document,
            &native.identity,
            PreparedNativeBounds {
                canonical_bytes: 65536,
                scopes: 4096,
                invariants: 4096,
            },
        )
        .unwrap();
        let (result, allocations) = allocation_probe::observe(|| run(&mut admission, &input));
        result.unwrap();
        assert_eq!(allocations.allocations, 0);
        assert_eq!(allocations.reallocations, 0);
        assert_eq!(admission.admitted().unwrap().canonical_bytes(), input);
        assert_eq!(
            LanguageParserScorerChoiceQuery::decode(admission.retained_bytes()).unwrap(),
            query
        );
        assert!(admission.actual_occurrences() > 0);
        assert!(admission.actual_occurrences() < 4096);
        // Corrupt an actual nested lexical span without changing canonical Type.
        let mut invalid = input.clone();
        let root = validate_canonical_structured_value(&invalid).unwrap();
        let token = field(field(field(root, "lexical"), "tape"), "tokens")
            .collection_index(0)
            .unwrap()
            .unwrap();
        let start = field(field(token, "span"), "start")
            .primitive_representation_bytes("value/u64")
            .unwrap();
        let offset = start.as_ptr() as usize - invalid.as_ptr() as usize;
        invalid[offset..offset + 8].copy_from_slice(&100u64.to_le_bytes());
        let mut rejected = PreparedNativeAdmission::new(
            &document,
            &native.identity,
            PreparedNativeBounds {
                canonical_bytes: 65536,
                scopes: 4096,
                invariants: 4096,
            },
        )
        .unwrap();
        let (result, allocations) = allocation_probe::observe(|| run(&mut rejected, &invalid));
        let PreparedNativeRefusal::Invariant { scope, .. } = result.unwrap_err() else {
            panic!()
        };
        assert_eq!(rejected.scope_native_type(scope).unwrap().name, "TextSpan");
        assert_eq!(allocations.allocations, 0);
        assert_eq!(allocations.reallocations, 0);
        assert!(rejected.admitted().is_err());
        assert_eq!(rejected.retained_bytes(), invalid);
        assert!(LanguageParserScorerChoiceQuery::decode(&invalid).is_err());
    }
}

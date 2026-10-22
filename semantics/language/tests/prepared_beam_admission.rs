//! Prepared Native admission prerequisite; no streaming/currentness acceptance.
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
const BYTES: usize = 65536;
const NAMES: [&str; 4] = ["candidate0", "candidate1", "candidate2", "candidate3"];
fn bounds() -> PreparedNativeBounds {
    PreparedNativeBounds {
        canonical_bytes: BYTES,
        scopes: 4096,
        invariants: 4096,
    }
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> ValidatedCanonicalStructuredValue<'a> {
    value.record_field(name).unwrap().unwrap()
}
struct Slot {
    state: PreparedStructuredComposer,
    parser: PreparedStructuredComposer,
    scorer: PreparedStructuredComposer,
    output: Vec<u8>,
}
impl Slot {
    fn new() -> Self {
        Self {
            state: PreparedStructuredComposer::new(
                &LanguageParserState::semantic_type().unwrap(),
                BYTES,
            )
            .unwrap(),
            parser: PreparedStructuredComposer::new(
                &LanguageParserHypothesis::semantic_type().unwrap(),
                BYTES,
            )
            .unwrap(),
            scorer: PreparedStructuredComposer::new(
                &LanguageParserScorerHypothesis::semantic_type().unwrap(),
                BYTES,
            )
            .unwrap(),
            output: Vec::with_capacity(BYTES),
        }
    }
    fn compose(&mut self, raw: ValidatedCanonicalStructuredValue<'_>) {
        let parser = field(raw, "parser");
        let state = field(parser, "state");
        let fields = [
            "basis",
            "committed",
            "depth",
            "heads",
            "relation0",
            "relation1",
            "relation2",
            "relation3",
            "stack",
            "token_count",
            "unread",
        ]
        .map(|name| field(state, name));
        let state = self.state.record(&fields).unwrap();
        let state = validate_canonical_structured_value(state).unwrap();
        let parser_bytes = self
            .parser
            .record(&[
                field(parser, "active"),
                field(parser, "identity"),
                field(parser, "score"),
                state,
            ])
            .unwrap();
        let parser = validate_canonical_structured_value(parser_bytes).unwrap();
        let bytes = self
            .scorer
            .record(&[field(raw, "choices"), parser, field(raw, "selected")])
            .unwrap();
        self.output.clear();
        self.output.extend_from_slice(bytes);
    }
}
struct BeamComposition {
    slots: [Slot; 4],
    beam: PreparedStructuredComposer,
}
impl BeamComposition {
    fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| Slot::new()),
            beam: PreparedStructuredComposer::new(
                &LanguageParserScorerBeam::semantic_type().unwrap(),
                BYTES,
            )
            .unwrap(),
        }
    }
    fn compose(&mut self, raw: &[u8]) -> &[u8] {
        let raw = validate_canonical_structured_value(raw).unwrap();
        for (slot, name) in self.slots.iter_mut().zip(NAMES) {
            slot.compose(field(raw, name));
        }
        let values = self
            .slots
            .each_ref()
            .map(|slot| validate_canonical_structured_value(&slot.output).unwrap());
        self.beam.record(&values).unwrap()
    }
}
fn finish(admission: &mut PreparedNativeAdmission<'_>) -> Result<(), PreparedNativeRefusal> {
    loop {
        if admission.step()? == PreparedNativeProgress::Admitted {
            return Ok(());
        }
    }
}
// Adversarial fixture mutation changes only an existing U64 payload, retaining
// all original canonical schema bytes. This does not construct Native authority.
fn change(raw: &mut [u8], slot: &str, name: &str, index: Option<u16>, number: u64) {
    let value = validate_canonical_structured_value(raw).unwrap();
    let candidate = field(value, slot);
    let value = if name == "selected" {
        field(candidate, name)
    } else {
        let value = field(field(field(candidate, "parser"), "state"), name);
        if let Some(index) = index {
            value.collection_index(index).unwrap().unwrap()
        } else {
            value
        }
    };
    let bytes = value.primitive_representation_bytes("value/u64").unwrap();
    let offset = bytes.as_ptr() as usize - raw.as_ptr() as usize;
    assert_eq!(bytes.len(), 8);
    raw[offset..offset + 8].copy_from_slice(&number.to_le_bytes());
}
fn checked_source() -> CheckedSyntaxDocument {
    let source = parse_syntax_document(language_semantic_source());
    assert_eq!(
        source.source_document_id().as_str(),
        parser_commitment::COMMITMENT_SOURCE_ID
    );
    let mut startup = StartupCatalog::new();
    startup
        .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
        .unwrap();
    check_syntax_document(&source, &startup).unwrap()
}
#[test]
fn actual_initial_beam_field_composition_preserves_all_four_nested_native_law_scopes() {
    let mut graph = parser_selection_io::SourceGraph::language("language-parser-initialize");
    let source = fixture::revision("answer record ", None, 14);
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new("analysis/native-beam".into()).unwrap(),
        source.material().revision().clone(),
        source.material().identity().clone(),
    )
    .unwrap();
    let begin = LanguageParserBegin::new(basis, fixture::dep(), 2).unwrap();
    // Actual current Source/Plan/Play numeric result, then exact existing Native
    // state admission. The Beam only supplies the existing active/inactive slots.
    let observed = graph.run(begin);
    let numeric = LanguageParserNumericState::from_structured(observed).unwrap();
    let state = fixture::native_state(&numeric);
    let inactive = LanguageParserScorerHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(false, 2, 0, state.clone()).unwrap(),
        *state.unread(),
    )
    .unwrap();
    let seed = LanguageParserScorerHypothesis::new(
        [0; 4],
        LanguageParserHypothesis::new(true, 1, 0, state).unwrap(),
        *inactive.selected(),
    )
    .unwrap();
    let initial =
        LanguageParserScorerBeam::new(seed, inactive.clone(), inactive.clone(), inactive).unwrap();
    let raw = initial.encode().unwrap();
    let expected = raw.clone();
    let document = checked_source();
    let native = document
        .native_types
        .iter()
        .find(|v| v.name == "LanguageParserScorerBeam")
        .unwrap();
    assert_eq!(
        native.value_type,
        LanguageParserScorerBeam::semantic_type().unwrap()
    );
    let mut admission =
        PreparedNativeAdmission::new(&document, &native.identity, bounds()).unwrap();
    let state = document
        .native_types
        .iter()
        .find(|v| v.name == "LanguageParserState")
        .unwrap();
    assert_eq!(
        (0..admission.scope_count())
            .filter(|i| admission.scope_native_type(*i).unwrap().identity == state.identity)
            .count(),
        4
    );
    let mut composer = BeamComposition::new();
    let ((), allocations) = allocation_probe::observe(|| {
        let composed = composer.compose(&raw);
        assert_eq!(composed, expected);
        admission.begin(composed).unwrap();
        finish(&mut admission).unwrap();
        assert_eq!(admission.admitted().unwrap().canonical_bytes(), expected);
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(
        LanguageParserScorerBeam::decode(admission.retained_bytes()).unwrap(),
        initial
    );
    // Even inactive slots retain the complete graph contract: corrupt each
    // slot's root anchor independently, not merely the active candidate.
    for name in NAMES {
        let mut invalid = raw.clone();
        change(&mut invalid, name, "heads", Some(4), 3);
        refuses_state(&document, native, state, &mut composer, &invalid);
    }
    // In-range cyclic edges; outer scorer selection remains internally legal.
    let mut cycle = raw.clone();
    for (name, index, value) in [
        ("unread", None, 2),
        ("heads", Some(0), 1),
        ("heads", Some(1), 0),
        ("selected", None, 2),
    ] {
        change(&mut cycle, "candidate3", name, index, value);
    }
    refuses_state(&document, native, state, &mut composer, &cycle);
    let mut committed = raw.clone();
    for (name, value) in [("unread", 1), ("committed", 1), ("selected", 1)] {
        change(&mut committed, "candidate2", name, None, value);
    }
    refuses_state(&document, native, state, &mut composer, &committed);
    let mut cancelled =
        PreparedNativeAdmission::new(&document, &native.identity, bounds()).unwrap();
    let composed = composer.compose(&raw);
    assert_eq!(
        cancelled.begin(&vec![0; BYTES + 1]),
        Err(PreparedNativeRefusal::Bounds)
    );
    assert!(cancelled.retained_bytes().is_empty());
    cancelled.begin(composed).unwrap();
    cancelled.step().unwrap();
    cancelled.cancel();
    assert_eq!(cancelled.step(), Err(PreparedNativeRefusal::Cancelled));
    assert_eq!(cancelled.retained_bytes(), composed);
    assert!(cancelled.admitted().is_err());
}
fn refuses_state(
    document: &CheckedSyntaxDocument,
    native: &CheckedNativeType,
    state: &CheckedNativeType,
    composer: &mut BeamComposition,
    raw: &[u8],
) {
    let mut admission = PreparedNativeAdmission::new(document, &native.identity, bounds()).unwrap();
    let (reason, allocations) = allocation_probe::observe(|| {
        let bytes = composer.compose(raw);
        admission.begin(bytes).unwrap();
        finish(&mut admission).unwrap_err()
    });
    let PreparedNativeRefusal::Invariant { scope, .. } = reason else {
        panic!("not a Source invariant refusal: {reason:?}")
    };
    assert_eq!(
        admission.scope_native_type(scope).unwrap().identity,
        state.identity
    );
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert_eq!(admission.retained_bytes(), composer.compose(raw));
    assert!(LanguageParserScorerBeam::decode(admission.retained_bytes()).is_err());
    assert!(admission.admitted().is_err());
}

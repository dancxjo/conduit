#![allow(dead_code)]
use conduit_core::revision::*;
use conduit_language::{
    analysis_epoch_initialization::*, analysis_epoch_owner::*, lexical::*, revision::*, *,
};
use conduit_plot::{
    rust_binding::{BoundedSequence, NativeRustBinding},
    PortableExpressionProgram,
};
pub fn text(s: &str) -> RevisionText<'_> {
    RevisionText::new(s).unwrap()
}
pub fn lexical() -> PreparedLexicalTape {
    let provenance =
        LinguisticDerivationProvenance::deterministic_rule("epoch-fixture".into(), "1".into())
            .unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("epoch-source".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("1".into()).unwrap(),
            "Hello".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        Some(5),
    )
    .unwrap();
    let entry = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
            "hello".into(),
            BoundedSequence::new(),
            LanguageLexicalPos::Interjection,
        )
        .unwrap()])
        .unwrap(),
        "Hello".into(),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([entry]).unwrap(),
        "epoch-fixture".into(),
        source.material().language().clone(),
        provenance,
    )
    .unwrap();
    prepare_lexical_tape(&source, &profile, None).unwrap()
}
pub fn request(tape: &PreparedLexicalTape) -> LanguageParserBegin {
    LanguageParserBegin::new(
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
            tape.tape().source().material().revision().clone(),
            tape.tape().source().material().identity().clone(),
        )
        .unwrap(),
        LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Dep,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap(),
        1,
    )
    .unwrap()
}
pub fn observed(request: &LanguageParserBegin) -> LanguageParserNumericState {
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/analysis_initialize_program.hex"
    )))
    .unwrap();
    LanguageParserNumericState::decode(
        &program
            .evaluate(&request.clone().encode().unwrap())
            .unwrap(),
    )
    .unwrap()
}

pub fn initialized<'a>(
    domain: &'a DependencyRevisions<'a>,
    context: RevisionContext<'a>,
    limits: RevisionLimits,
) -> AnalysisEpochOwner<'a> {
    let tape = lexical();
    let request = request(&tape);
    let actual = observed(&request);
    let program = PreparedAnalysisInitializationProgram::prepare().unwrap();
    let initial = program.replay(tape, &request, &actual).unwrap();
    AnalysisEpochOwner::initialize(initial, domain, context, limits).unwrap()
}

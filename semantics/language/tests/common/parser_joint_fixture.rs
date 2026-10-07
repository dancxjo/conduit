#![allow(dead_code)]
use super::{fixture, scorer_model};
use conduit_core::*;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
pub const BYTES: &[u8] = include_bytes!("../../training/ewt_joint/ewt_joint.i16");
pub const MANIFEST: &str = include_str!("../../training/ewt_joint/manifest.json");
pub const PROFILE: &[u8] = include_bytes!("../../training/ewt_joint/lexical_profile.json");
#[derive(serde::Deserialize)]
pub struct Sentence {
    pub id: String,
    pub forms: Vec<String>,
    pub pos: Vec<u64>,
    pub heads: Vec<u64>,
    pub relations: Vec<String>,
}
pub fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn pos_source() -> String {
    [
        fixture::parser_source(),
        include_str!("../../text_revision.conduit").into(),
        include_str!("../../lexical.conduit").into(),
        include_str!("../../parser_joint.conduit").into(),
    ]
    .join("\n")
}
pub fn source() -> String {
    [
        fixture::parser_source(),
        include_str!("../../text_revision.conduit").into(),
        include_str!("../../lexical.conduit").into(),
        include_str!("../../parser_joint.conduit").into(),
        include_str!("../../parser_joint_decode.conduit").into(),
    ]
    .join("\n")
}
pub fn scorer() -> scorer_model::Scorer {
    let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let joint = hex(semantic_digest(
        "language/parser-joint-encoding@1",
        include_bytes!("../../parser_joint.conduit"),
    ));
    let profile = hex(semantic_digest(
        "language/parser-lexical-profile@1",
        PROFILE,
    ));
    assert_eq!(manifest["joint_choice_contract_identity"], joint);
    assert_eq!(manifest["lexical_profile_identity"], profile);
    scorer_model::Scorer::prepare(BYTES, MANIFEST, &format!("joint/{joint}/{profile}"))
}
pub fn retype(ty: &StructuredInfoType, value: &StructuredInfoValue) -> StructuredInfoValue {
    match (ty.shape(), value.shape()) {
        (StructuredInfoTypeShape::Record { fields, .. }, StructuredInfoValueShape::Record(_)) => {
            fixture::record(
                ty,
                fields
                    .iter()
                    .map(|f| {
                        (
                            f.name(),
                            retype(f.value_type(), fixture::field(value, f.name())),
                        )
                    })
                    .collect(),
            )
        }
        _ => value.clone(),
    }
}
fn pos_codes() -> [LanguageLexicalPos; 17] {
    // Official corpus metadata ingestion. Runtime numerical code projection is
    // the independently checked Source graph, exercised for all17 alternatives.
    [
        LanguageLexicalPos::Adjective,
        LanguageLexicalPos::Adposition,
        LanguageLexicalPos::Adverb,
        LanguageLexicalPos::Auxiliary,
        LanguageLexicalPos::CoordinatingConjunction,
        LanguageLexicalPos::Determiner,
        LanguageLexicalPos::Interjection,
        LanguageLexicalPos::Noun,
        LanguageLexicalPos::Numeral,
        LanguageLexicalPos::Particle,
        LanguageLexicalPos::Pronoun,
        LanguageLexicalPos::ProperNoun,
        LanguageLexicalPos::Punctuation,
        LanguageLexicalPos::SubordinatingConjunction,
        LanguageLexicalPos::Symbol,
        LanguageLexicalPos::Verb,
        LanguageLexicalPos::Other,
    ]
}
pub fn lexical(sentence: &Sentence) -> Result<LanguageParserJointLexical, &'static str> {
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "ud/ewt-train-profile".into(),
        "ud/ewt-2.18".into(),
    )
    .unwrap();
    let language = LanguageId::new("language/en".into()).unwrap();
    let profile_data: std::collections::BTreeMap<String, Vec<usize>> =
        serde_json::from_slice(PROFILE).unwrap();
    let pos = pos_codes();
    let entries =
        BoundedSequence::try_from_iter(profile_data.into_iter().map(|(surface, codes)| {
            let candidates = BoundedSequence::try_from_iter(codes.into_iter().map(|code| {
                LanguageLexicalCandidate::new(surface.clone(), BoundedSequence::new(), pos[code])
                    .unwrap()
            }))
            .unwrap();
            LanguageLexicalEntry::new(candidates, surface).unwrap()
        }))
        .unwrap();
    let profile_identity = hex(semantic_digest(
        "language/parser-lexical-profile@1",
        PROFILE,
    ));
    let profile = LanguageLexicalProfile::new(
        entries,
        profile_identity,
        language.clone(),
        provenance.clone(),
    )
    .unwrap();
    let material = sentence.forms.join(" ");
    let identity = hex(semantic_digest(
        "ud/ewt-derived-occurrence@1",
        format!("{}/{}", sentence.id, material).as_bytes(),
    ));
    let source_revision = hex(semantic_digest(
        "ud/ewt-derived-source@1",
        material.as_bytes(),
    ));
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new(identity).unwrap(),
            language,
            LanguageTextRevisionId::new(source_revision).unwrap(),
            material,
        )
        .unwrap(),
        None,
        provenance,
        0,
        None,
    )
    .unwrap();
    let prepared =
        prepare_lexical_tape(&source, &profile, None).map_err(|_| "canonical_lexer_refusal")?;
    let tape = prepared.tape();
    if tape.tokens().len() != sentence.forms.len()
        || tape
            .tokens()
            .iter()
            .zip(&sentence.forms)
            .any(|(t, w)| t.surface() != w)
    {
        return Err("canonical_tokenization_mismatch");
    }
    if tape.tokens().iter().any(|t| t.candidates().is_empty()) {
        return Err("canonical_profile_lookup_missing");
    }
    LanguageParserJointLexical::new(tape.clone(), sentence.forms.len() as u64)
        .map_err(|_| "native_joint_lexical_refusal")
}
pub fn initial(
    f: &mut fixture::Fixture,
    lexical: &LanguageParserJointLexical,
    model: &scorer_model::Scorer,
) -> LanguageParserState {
    let material = lexical.tape().source().material();
    let basis = LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new(analysis_identity(lexical, model)).unwrap(),
        material.revision().clone(),
        material.identity().clone(),
    )
    .unwrap();
    let raw = fixture::replace(
        &f.initial(*lexical.token_count()),
        "basis",
        basis.into_structured().unwrap(),
    );
    LanguageParserState::from_structured(retype(
        &LanguageParserState::semantic_type().unwrap(),
        &raw,
    ))
    .unwrap()
}
pub fn inactive(state: &LanguageParserState) -> LanguageParserJointRuntimeHypothesis {
    LanguageParserJointRuntimeHypothesis::new(
        LanguageParserJointHypothesis::new(
            [0; 4],
            LanguageParserHypothesis::new(false, 0, 0, state.clone()).unwrap(),
        )
        .unwrap(),
        0,
    )
    .unwrap()
}
pub fn empty(state: &LanguageParserState) -> LanguageParserJointRuntimeRawBeam {
    let candidate = inactive(state);
    LanguageParserJointRuntimeRawBeam::new(
        candidate.clone(),
        candidate.clone(),
        candidate.clone(),
        candidate,
    )
    .unwrap()
}
pub fn admit(
    beam: &LanguageParserJointRuntimeRawBeam,
    lexical: &LanguageParserJointLexical,
    basis: &LanguageParserBasis,
    invocation: u64,
) -> LanguageParserJointRuntimeBeam {
    let candidates = slots(beam);
    let joint = LanguageParserJointBeam::new(
        basis.clone(),
        candidates[0].hypothesis().clone(),
        candidates[1].hypothesis().clone(),
        candidates[2].hypothesis().clone(),
        candidates[3].hypothesis().clone(),
        *lexical.tape().source().sequence(),
        invocation,
        lexical.clone(),
    )
    .unwrap();
    LanguageParserJointRuntimeBeam::new(joint, candidates.map(|c| *c.selected())).unwrap()
}
pub fn slots(
    beam: &LanguageParserJointRuntimeRawBeam,
) -> [&LanguageParserJointRuntimeHypothesis; 4] {
    [
        beam.candidate0(),
        beam.candidate1(),
        beam.candidate2(),
        beam.candidate3(),
    ]
}
pub fn admit_arc(proposal: &StructuredInfoValue) -> LanguageDependencyArc {
    let basis = fixture::field(proposal, "basis");
    let token = |ordinal| {
        LanguageAnalysisTokenRef::new(
            LanguageAnalysisRevisionId::from_structured(
                fixture::field(basis, "analysis_revision").clone(),
            )
            .unwrap(),
            LinguisticTokenIdentity::new(
                ordinal,
                LanguageTextId::from_structured(fixture::field(basis, "text").clone()).unwrap(),
                LanguageTextRevisionId::from_structured(
                    fixture::field(basis, "source_revision").clone(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let head = fixture::count(fixture::field(proposal, "head"));
    let governor = if head == 4 {
        LanguageDependencyHead::Root
    } else {
        let token = token(head);
        LanguageDependencyHead::token(token.revision().clone(), token.token().clone()).unwrap()
    };
    let base = LanguageUniversalDependencyRelation::from_structured(
        fixture::field(fixture::field(proposal, "relation"), "base").clone(),
    )
    .unwrap();
    // The current learned class contract predicts universal bases and has no
    // subtype label. Native canonical relation/arc laws are the final boundary.
    LanguageDependencyArc::new(
        token(fixture::count(fixture::field(proposal, "dependent"))),
        governor,
        LanguageDependencyRelation::new(base, None).unwrap(),
    )
    .unwrap()
}

pub fn analysis_identity(
    lexical: &LanguageParserJointLexical,
    model: &scorer_model::Scorer,
) -> String {
    let policy = semantic_digest("language/parser-joint-source-policy@1", source().as_bytes());
    let tape = lexical
        .tape()
        .clone()
        .into_structured()
        .unwrap()
        .semantic_digest()
        .unwrap();
    let components = [
        model.artifact.content_identity(),
        model.artifact.signature_identity,
        policy,
        tape,
    ]
    .concat();
    hex(semantic_digest(
        "language/parser-joint-analysis-contract@1",
        &components,
    ))
}

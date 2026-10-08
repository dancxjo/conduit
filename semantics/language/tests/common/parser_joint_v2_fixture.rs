#![allow(dead_code)]
use super::{fixture, scorer_model};
use conduit_core::*;
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
pub const BYTES: &[u8] = include_bytes!("../../training/ewt_joint_v2/ewt_joint.i16");
pub const MANIFEST: &str = include_str!("../../training/ewt_joint_v2/manifest.json");
pub const PROFILE: &[u8] = include_bytes!("../../training/ewt_joint_v2/lexical_profile.json");
#[derive(serde::Deserialize)]
pub struct Sentence {
    pub id: String,
    pub forms: Vec<String>,
    #[serde(default)]
    pub text: Option<String>,
    pub pos: Vec<u64>,
    pub heads: Vec<u64>,
    pub relations: Vec<String>,
}
pub fn sentence_text(sentence: &Sentence) -> String {
    sentence
        .text
        .clone()
        .unwrap_or_else(|| sentence.forms.join(" "))
}
pub fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn pos_source() -> String {
    [
        fixture::parser_source(),
        include_str!("../../text_revision.conduit").into(),
        include_str!("../../lexical.conduit").into(),
        include_str!("../../parser_available.conduit").into(),
        include_str!("../../parser_scorer_v2.conduit").into(),
    ]
    .join("\n")
}
pub fn source() -> String {
    [
        fixture::parser_source(),
        include_str!("../../text_revision.conduit").into(),
        include_str!("../../lexical.conduit").into(),
        include_str!("../../parser_joint.conduit").into(),
        include_str!("../../revision_lineage.conduit").into(),
        include_str!("../../parser_available.conduit").into(),
        include_str!("../../parser_scorer_v2.conduit").into(),
        include_str!("../../parser_joint_decode.conduit").into(),
    ]
    .join("\n")
}
pub fn runtime_source() -> String {
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
    let available = hex(semantic_digest(
        "language/parser-available-contract@1",
        include_bytes!("../../parser_available.conduit"),
    ));
    assert_eq!(manifest["availability_contract_identity"], available);
    scorer_model::Scorer::prepare_v2(BYTES, MANIFEST, &format!("v2/{available}/{profile}"))
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
fn curated_lexical(sentence: &Sentence) -> Result<LanguageParserJointLexical, &'static str> {
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "language/parser-joint-v2-reviewed-and-ewt".into(),
        "ud/ewt-2.18+reviewed-vocative@1".into(),
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
    let material = sentence_text(sentence);
    let identity = hex(semantic_digest(
        "language/parser-evaluation-occurrence@1",
        format!("{}/{}", sentence.id, material).as_bytes(),
    ));
    let source_revision = hex(semantic_digest(
        "language/parser-joint-v2-source@1",
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
pub fn acquisition_history(id: &str) -> Option<serde_json::Value> {
    let path = std::env::var("CONDUIT_PARSER_JOINT_V2_ASR_SOURCES").ok()?;
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    Some(
        rows.into_iter()
            .find(|row| row["id"] == id)
            .expect("exact occurrence"),
    )
}
pub fn lexical(sentence: &Sentence) -> Result<LanguageParserJointLexical, &'static str> {
    let curated = curated_lexical(sentence)?;
    let Some(history) = acquisition_history(&sentence.id) else {
        return Ok(curated);
    };
    let bytes: Vec<u8> = serde_json::from_value(history["lexical_tape_bytes"].clone()).unwrap();
    let tape = LanguageLexicalTape::from_structured(
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
    )
    .unwrap();
    assert_eq!(
        tape.profile(),
        curated.tape().profile(),
        "exact reviewed profile"
    );
    assert_eq!(
        tape.source().material().text().as_str(),
        sentence_text(sentence)
    );
    assert_eq!(history["forms"], serde_json::json!(sentence.forms));
    let revisions = history["language_revision_bytes"].as_array().unwrap();
    assert_eq!(revisions.len(), 3);
    assert_eq!(history["asr_envelope_bytes"].as_array().unwrap().len(), 3);
    let mut previous = None;
    for encoded in revisions {
        let bytes: Vec<u8> = serde_json::from_value(encoded.clone()).unwrap();
        let revision = LanguageTextRevision::from_structured(
            StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
        )
        .unwrap();
        previous =
            Some(prepare_lexical_tape(&revision, tape.profile(), previous.as_ref()).unwrap());
    }
    assert_eq!(
        previous.unwrap().tape(),
        &tape,
        "canonical full revision-chain replay"
    );
    LanguageParserJointLexical::new(tape, sentence.forms.len() as u64)
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
    initial_from_basis(f, *lexical.token_count(), basis)
}
pub fn initial_from_basis(
    f: &mut fixture::Fixture,
    token_count: u64,
    basis: LanguageParserBasis,
) -> LanguageParserState {
    let raw = fixture::replace(
        &f.initial(token_count),
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
        *state.unread(),
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

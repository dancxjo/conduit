//! Exact version3 resource/profile custody. No graph or accuracy is inferred here.
use conduit_ai::{integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
use conduit_data::*;
use conduit_language::{parser_model_selection::*, *};
use conduit_plot::rust_binding::BoundedSequence;
use std::sync::Arc;
pub fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|value| format!("{value:02x}")).collect()
}
pub fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::model(
        "window8/ewt-reviewed-train".into(),
        "version3/425-76-25".into(),
    )
    .unwrap()
}
pub fn lexical(bytes: &[u8]) -> LanguageLexicalProfile {
    let data: std::collections::BTreeMap<String, Vec<usize>> =
        serde_json::from_slice(bytes).unwrap();
    let pos = [
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
    ];
    let entries = data.into_iter().map(|(word, codes)| {
        let candidates = codes.into_iter().map(|code| {
            LanguageLexicalCandidate::new(word.clone(), BoundedSequence::new(), pos[code].clone())
                .unwrap()
        });
        LanguageLexicalEntry::new(BoundedSequence::try_from_iter(candidates).unwrap(), word)
            .unwrap()
    });
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        hex(semantic_digest("language/parser-lexical-profile@1", bytes)),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}
pub fn feature_contract() -> [u8; 32] {
    semantic_digest(
        "language/parser-v3-window8-scorer-encoding@1",
        include_bytes!("../../parser_window8.conduit"),
    )
}
pub fn signature() -> ModelSignature {
    let encoding = hex(feature_contract());
    let port = |name: &str, element, count| {
        ModelPortConstraint::new(
            ModelPortIdentity::new(name.into()).unwrap(),
            ModelPortPresence::Required,
            ModelSemanticKind::new(format!("language/parser-{name}/{encoding}@3")).unwrap(),
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(
                    vec![element],
                    vec![ModelAxisConstraint::new(
                        ModelDimensionConstraint::fixed(count).unwrap(),
                        TensorAxisRole::Feature,
                    )
                    .unwrap()],
                    count * 8,
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    ModelSignature::from_parts(
        format!("language/parser-window8/{encoding}@3"),
        3,
        vec![ModelOperation::Infer],
        vec![port("features", TensorElement::U64, 25)],
        vec![port("scores", TensorElement::I64, 76)],
    )
    .unwrap()
}
pub fn declaration(
    model: &Arc<PreparedCategoricalStep>,
    lexical: &LanguageLexicalProfile,
) -> (Arc<ParserModelProfileDefinition>, ParserSourceModelContract) {
    let source = include_bytes!("../../parser_window8.conduit");
    let contracts = ParserSourceModelContract {
        feature_contract: feature_contract(),
        availability_contract: semantic_digest("language/parser-window8-available@3", source),
        action_contract: semantic_digest("language/parser-window8-action@3", source),
        joint_choice_contract: semantic_digest(
            "language/parser-window8-choice-search@3",
            include_bytes!("../../parser_window8_search.conduit"),
        ),
        numeric_indices_contract: model.indices_type().semantic_digest().unwrap(),
        numeric_scores_contract: model.scores_type().semantic_digest().unwrap(),
    };
    let definition = ParserModelProfileDefinition::new(
        "language/parser-window8/declared@3".into(),
        3,
        lexical.clone(),
        provenance(),
        contracts.clone(),
        model.resource().artifact().clone(),
        model.resource().signature().clone(),
        (425, 76, 25),
        1000000,
    )
    .unwrap();
    (Arc::new(definition), contracts)
}
pub fn source(model: &PreparedCategoricalStep) -> String {
    let contract = model.contract(true).unwrap();
    let KindSemanticLaw::ValueContracts(values) = &contract.semantic_laws[0] else {
        panic!("numerical contract")
    };
    let bytes = values
        .iter()
        .find(|value| value.location == FrontValueLocation::Output(port_id("scores")))
        .unwrap()
        .contract
        .maximum_bytes;
    [super::fixture::parser_source(),include_str!("../../text_revision.conduit").into(),include_str!("../../lexical.conduit").into(),include_str!("../../parser_window8.conduit").into(),include_str!("../../parser_window8_search.conduit").into(),format!("plot window8-numeric-projection (\n features: LanguageParserWindow8RawModelFeatures...| >> indices: LanguageParserCategoricalIndices...|\n) = .indices\nplot window8-learned-model (\n features: LanguageParserWindow8RawModelFeatures...| >> scores: LanguageParserCategoricalScores...| <= {bytes}B\n) {{\n projection: window8-numeric-projection\n model: {}\n features >> projection.features\n projection.indices >> model.indices\n model.scores >> scores\n}}\n",model.kind_identity(true))].join("\n")
}

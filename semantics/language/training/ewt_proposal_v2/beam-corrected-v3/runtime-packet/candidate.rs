//! Test-only independently pinned candidate; no production factory authority.
use super::model_resource;
use conduit_ai::{
    integer_categorical_step::PreparedCategoricalStep, ModelArtifact, ModelSignature,
};
use conduit_core::*;
use conduit_language::{
    lexical_proposer_port::{
        token_producer::*,
        token_producer::{model_definition::*, revision::*},
        *,
    },
    lexical_proposer_resource as resource,
    parser_model_selection::ParserSourceModelContract,
    *,
};
use conduit_plot::rust_binding::{NativeRustBinding, PreparedNativeFamilyLimits};
use std::{path::Path, sync::Arc};
pub struct Candidate {
    pub owner: PreparedRevisionProducer,
    pub scorer: Arc<PreparedCategoricalStep>,
    pub selected: PreparedProposalModelSelection,
    pub contracts: ParserSourceModelContract,
}
fn native() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 256,
        maximum_input_bytes: 262144,
        maximum_retained_bytes: 200_000_000,
        maximum_preparation_peak_bytes: 1_000_000_000,
        maximum_conversion_requested_bytes: 1_000_000_000,
    }
}
fn limits() -> resource::LexicalDictionaryLimits {
    resource::LexicalDictionaryLimits {
        maximum_shards: 128,
        maximum_entries: 8192,
        maximum_frame_bytes: 262144,
        maximum_retained_bytes: 300_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        native: native(),
    }
}
fn producer_limits() -> TokenProducerLimits {
    TokenProducerLimits {
        maximum_retained_bytes: 300_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        maximum_admission_peak_bytes: 2_000_000_000,
        maximum_response_retained_bytes: 10_000_000,
    }
}
fn revision_limits() -> RevisionLimits {
    RevisionLimits {
        maximum_tokens: 128,
        maximum_retained_bytes: 1_000_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        maximum_admission_peak_bytes: 3_000_000_000,
        maximum_response_retained_bytes: 1_000_000_000,
    }
}
pub fn prepare() -> Candidate {
    let archive = &Path::new(env!("CARGO_MANIFEST_DIR")).join("training/ewt_proposal_v1");
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("/home/dancxjo/conduit-4907-native-reuse/work/beam-training-candidate/expected-metadata-new");
    let shards = (0..128)
        .map(|n| {
            std::fs::read(archive.join(format!(
                "canonical-dictionary-overlay-v2/shard-{n:03}.native.bin"
            )))
            .unwrap()
        })
        .collect();
    let dictionary =
        resource::PreparedLexicalDictionary::prepare(shards, "language/en", limits()).unwrap();
    assert_eq!(
        dictionary
            .identity()
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>(),
        "12458ed85a62ff19a794a8f79e24ff8c7e76e3975392379ed3ba9ed5c9ec89fe"
    );
    let proposer = std::fs::read(expected.join("proposer-definition.native.bin")).unwrap();
    let port = PreparedLexicalProposerPort::prepare(
        dictionary,
        &proposer,
        LexicalProposerPortLimits {
            maximum_frame_bytes: 262144,
            maximum_retained_bytes: 300_000_000,
            maximum_preparation_peak_bytes: 2_000_000_000,
            maximum_admission_peak_bytes: 2_000_000_000,
            maximum_response_requested_bytes: 1_000_000_000,
            native: native(),
        },
    )
    .unwrap();
    let owner = lexical_proposer_port::token_producer::revision::PreparedRevisionProducer::prepare(
        PreparedTokenProducer::prepare(port, producer_limits()).unwrap(),
        revision_limits(),
    )
    .unwrap();
    let signature =
        ModelSignature::decode(&std::fs::read(expected.join("signature.native.bin")).unwrap())
            .unwrap();
    let reference = BoundedResourceRef::decode(
        &std::fs::read(expected.join("artifact-reference.bin")).unwrap(),
    )
    .unwrap();
    let artifact = ModelArtifact {
        architecture_profile: conduit_ai::integer_categorical::CATEGORICAL_I16_ARCHITECTURE.into(),
        format_profile: conduit_ai::integer_categorical::CATEGORICAL_I16_FORMAT.into(),
        precision_profile: conduit_ai::integer_categorical::CATEGORICAL_I16_PRECISION.into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference,
    };
    assert_eq!(
        format!("{artifact:#?}"),
        std::fs::read_to_string(expected.join("artifact-full.debug.txt")).unwrap()
    );
    let indices = StructuredInfoType::from_canonical_bytes(
        &std::fs::read(expected.join("numeric-indices.type.bin")).unwrap(),
    )
    .unwrap();
    let scores = StructuredInfoType::from_canonical_bytes(
        &std::fs::read(expected.join("numeric-scores.type.bin")).unwrap(),
    )
    .unwrap();
    let source =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("parser_window8.conduit"))
            .unwrap();
    let search =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("parser_window8_search.conduit"))
            .unwrap();
    let feature = [
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("parser_window8_proposal_features.conduit"),
        )
        .unwrap(),
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("parser_window8_proposal_features_v2.conduit"),
        )
        .unwrap(),
    ]
    .concat();
    let contracts = conduit_language::parser_model_selection::ParserSourceModelContract {
        feature_contract: semantic_digest(
            "language/parser-proposal-window8-scorer-encoding@2",
            &feature,
        ),
        availability_contract: semantic_digest("language/parser-window8-available@3", &source),
        action_contract: semantic_digest("language/parser-window8-action@3", &source),
        joint_choice_contract: semantic_digest("language/parser-window8-choice-search@3", &search),
        numeric_indices_contract: indices.semantic_digest().unwrap(),
        numeric_scores_contract: scores.semantic_digest().unwrap(),
    };
    let pins = std::fs::read_to_string(expected.join("expected.json")).unwrap();
    for (name, digest) in [
        ("feature", contracts.feature_contract),
        ("availability", contracts.availability_contract),
        ("action", contracts.action_contract),
        ("joint_choice", contracts.joint_choice_contract),
        ("numeric_indices", contracts.numeric_indices_contract),
        ("numeric_scores", contracts.numeric_scores_contract),
    ] {
        let hex = digest
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        assert!(
            pins.contains(&format!("\"{name}\": \"{hex}\"")),
            "independent Source pin {name}"
        );
    }
    assert_eq!(
        core::char::UNICODE_VERSION,
        (17, 0, 0),
        "reviewed segmentation classifier version"
    );
    let training = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("/home/dancxjo/conduit-4907-native-reuse/semantics/language/training/ewt_proposal_v2/beam-corrected-v3/declared-model-training-manifest.json"),
    )
    .unwrap();
    let definition = Arc::new(
        ProposalModelDefinition::new(
            &owner,
            "TRAIN+22-reviewed-proposal-beam-corrected-v3".into(),
            "selected-history-origin/411-27-76@2".into(),
            contracts.clone(),
            artifact,
            signature.clone(),
            (411, 76, 27),
            1_000_000,
            training.into_boxed_slice(),
            262144,
        )
        .unwrap(),
    );
    let weights = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("/home/dancxjo/conduit-4907-native-reuse/semantics/language/training/ewt_proposal_v2/beam-corrected-v3/proposal_window8.i16"),
    )
    .unwrap();
    let scorer = model_resource::categorical(weights, signature, 1);
    let selected =
        PreparedProposalModelSelection::prepare(&owner, scorer.clone(), definition, &contracts)
            .unwrap();
    assert_eq!(selected.categorical().dimensions(), (411, 76, 27));
    assert!(std::ptr::eq(selected.categorical(), scorer.as_ref()));
    Candidate {
        owner,
        scorer,
        selected,
        contracts,
    }
}
/// Full original declarations and a distinct fixed numerical projection alias.
/// Model execution is through the retained original Plan/Play test target.
pub fn numeric_source(candidate: &Candidate) -> String {
    let contract = candidate.scorer.contract(true).unwrap();
    let KindSemanticLaw::ValueContracts(values) = &contract.semantic_laws[0] else {
        panic!("model contract")
    };
    let maximum = values
        .iter()
        .find(|value| value.location == FrontValueLocation::Output(port_id("scores")))
        .unwrap()
        .contract
        .maximum_bytes;
    [super::fixture::parser_source(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/text_revision.conduit").into(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/lexical.conduit").into(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/lexical_proposer.conduit").into(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8.conduit").into(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8_search.conduit").into(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8_proposal_features.conduit").into(),include_str!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8_proposal_features_v2.conduit").into(),format!("plot window8-proposal-v2-numeric-projection (\n features: LanguageParserProposalWindow8V2RawFeatures...| >> indices: LanguageParserWindow8ProposerV2CategoricalIndices...|\n) = .indices\nplot window8-proposal-v2-learned-model (\n features: LanguageParserProposalWindow8V2RawFeatures...| >> scores: LanguageParserWindow8ProposerV2CategoricalScores...| <= {maximum}B\n) {{\n projection: window8-proposal-v2-numeric-projection\n model: {}\n features >> projection.features\n projection.indices >> model.indices\n model.scores >> scores\n}}\n",candidate.scorer.kind_identity(true))].join("\n")
}

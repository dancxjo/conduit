#![allow(unused_imports,dead_code)]
use conduit_plot::rust_binding::NativeRustBinding;
// Exact version3 resource/profile custody. No graph or accuracy is inferred here.
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
            LanguageLexicalCandidate::new(word.clone(), BoundedSequence::new(), pos[code]).unwrap()
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
        "language/parser-proposal-window8-scorer-encoding@2",
        &[include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8_proposal_features.conduit").as_slice(), include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/work/lexical-proposer-next/proposal_window8_features_v2.conduit").as_slice()].concat(),
    )
}
pub fn signature() -> ModelSignature {
    let encoding = hex(feature_contract());
    let port = |name: &str, element, count| {
        ModelPortConstraint::new(
            ModelPortIdentity::new(name.into()).unwrap(),
            ModelPortPresence::Required,
            ModelSemanticKind::new(format!("language/parser-proposal-{name}/{encoding}@1")).unwrap(),
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
        format!("language/parser-proposal-window8-v2/{encoding}@2"),
        3,
        vec![ModelOperation::Infer],
        vec![port("features", TensorElement::U64, 27)],
        vec![port("scores", TensorElement::I64, 76)],
    )
    .unwrap()
}


fn main(){
 let out=std::path::Path::new("work/lexical-proposer-next/proposal27-v2-expected-metadata-v1");std::fs::create_dir(out).unwrap();
 let weights=std::fs::read("work/lexical-proposer-next/proposal-411-training-v1/proposal_window8.i16").unwrap();assert_eq!(weights.len(),62492);assert_eq!(&weights[..8],b"CI16SUM1");
 let signature=signature();std::fs::write(out.join("signature.native.bin"),signature.clone().encode().unwrap()).unwrap();
 let content=conduit_ai::model_content_digest(&weights);
 let artifact=ModelArtifact{architecture_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_ARCHITECTURE.into(),format_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_FORMAT.into(),precision_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_PRECISION.into(),state_schema_version:1,signature_identity:signature.semantic_digest().unwrap(),content:BoundedResourceRef{identity:ResourceSemanticIdentity::from_digest(content),content_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_FORMAT.into(),access_class:"model/pinned-training-artifact/read@1".into(),extent:ResourceExtent{bytes:62492,items:Some(1)},lifetime:ResourceLifetime{version:ResourceVersionIdentity::from_digest(content),expires_at:None}}};artifact.validate(&signature).unwrap();
 std::fs::write(out.join("artifact-reference.bin"),artifact.content.encode().unwrap()).unwrap();std::fs::write(out.join("artifact-full.debug.txt"),format!("{artifact:#?}")).unwrap();
 let policy_material=std::fs::read("work/lexical-proposer-next/unknown-policy-overlay-v2.json").unwrap();let policy_identity=hex(semantic_digest("language/proposal-unknown-policy@1",&policy_material));
 let provenance=LinguisticDerivationProvenance::deterministic_rule("TRAIN-frequency-unknown-candidates".into(),policy_identity.clone()).unwrap();
 let unknown=LanguageLexicalUnknownPolicy::new(BoundedSequence::try_from_iter([LanguageLexicalPos::Adjective,LanguageLexicalPos::Noun,LanguageLexicalPos::ProperNoun,LanguageLexicalPos::Verb]).unwrap(),LanguageLexicalUnknownCommitPolicy::StableInputConsensus,policy_identity,provenance).unwrap();
 std::fs::write(out.join("unknown-policy.native.bin"),unknown.clone().encode().unwrap()).unwrap();
 let dictionary="12458ed85a62ff19a794a8f79e24ff8c7e76e3975392379ed3ba9ed5c9ec89fe";
 let definition=LanguageLexicalProposerDefinition::new(dictionary.into(),hex(semantic_digest("language/proposal-definition@1",&[dictionary.as_bytes(),&policy_material].concat())),LanguageId::new("language/en".into()).unwrap(),LinguisticDerivationProvenance::model("TRAIN-plus-22-authored-proposals".into(),"411-76-27-candidate2".into()).unwrap(),unknown).unwrap();
 std::fs::write(out.join("proposer-definition.native.bin"),definition.encode().unwrap()).unwrap();
 let indices=StructuredInfoType::collection(StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),Some(27)).unwrap();let scores=StructuredInfoType::collection(StructuredInfoType::leaf(kind_id("value/i64")).unwrap(),Some(76)).unwrap();std::fs::write(out.join("numeric-indices.type.bin"),indices.canonical_bytes().unwrap()).unwrap();std::fs::write(out.join("numeric-scores.type.bin"),scores.canonical_bytes().unwrap()).unwrap();
 let source=include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8.conduit");let search=include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8_search.conduit");
 let manifest=serde_json::json!({"scope":"Independently derived expected metadata for pinned candidate before scorer adoption; no closed factory/runtime/accuracy authority","content_identity":hex(content),"signature_identity":hex(signature.semantic_digest().unwrap()),"artifact_descriptor":hex(artifact.descriptor_digest(&signature).unwrap()),"reference_identity":hex(artifact.content.semantic_digest().unwrap()),"dictionary_identity":dictionary,"dimensions":{"categories":411,"classes":76,"lookups":27},"maximum_score_magnitude":1000000,"contracts":{"feature":hex(feature_contract()),"availability":hex(semantic_digest("language/parser-window8-available@3",source)),"action":hex(semantic_digest("language/parser-window8-action@3",source)),"joint_choice":hex(semantic_digest("language/parser-window8-choice-search@3",search)),"numeric_indices":hex(indices.semantic_digest().unwrap()),"numeric_scores":hex(scores.semantic_digest().unwrap())}});std::fs::write(out.join("expected.json"),serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();println!("PASS expectedcontent={} signature={}",hex(content),hex(signature.semantic_digest().unwrap()));
}

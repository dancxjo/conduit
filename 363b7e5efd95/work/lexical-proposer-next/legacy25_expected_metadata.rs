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
        "language/parser-v3-window8-scorer-encoding@1",
        include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8.conduit"),
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

fn main(){
 let output=std::path::Path::new("work/lexical-proposer-next/legacy25-expected-metadata-v1");std::fs::create_dir(output).unwrap();
 let signature=signature();let signature_bytes=signature.clone().encode().unwrap();std::fs::write(output.join("signature.native.bin"),signature_bytes).unwrap();
 let expected_content=[28,225,10,200,143,1,154,147,58,140,135,43,81,249,114,115,203,237,155,24,238,8,13,17,236,46,242,45,149,54,180,142];
 let reference=BoundedResourceRef{identity:ResourceSemanticIdentity::from_digest(expected_content),content_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_FORMAT.into(),access_class:"model/pinned-training-artifact/read@1".into(),extent:ResourceExtent{bytes:64620,items:Some(1)},lifetime:ResourceLifetime{version:ResourceVersionIdentity::from_digest(expected_content),expires_at:None}};
 let artifact=ModelArtifact{architecture_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_ARCHITECTURE.into(),format_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_FORMAT.into(),precision_profile:conduit_ai::integer_categorical::CATEGORICAL_I16_PRECISION.into(),state_schema_version:1,signature_identity:signature.semantic_digest().unwrap(),content:reference};artifact.validate(&signature).unwrap();
 std::fs::write(output.join("artifact-reference.bin"),artifact.content.encode().unwrap()).unwrap();std::fs::write(output.join("artifact-full.debug.txt"),format!("{artifact:#?}")).unwrap();
 let profile_bytes=std::fs::read("/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/work/window8/v3-vocative-candidate-pinned-teaching/lexical_profile.json").unwrap();assert_eq!(hex(semantic_digest("language/parser-lexical-profile@1",&profile_bytes)),"7df46c1df70a0d296bf6ff5a41f0b920899451e8becb0dc9ba82fd98ac435469");
 let profile=lexical(&profile_bytes);let profile_canonical=profile.encode().unwrap();std::fs::write(output.join("lexical-profile.native.bin"),&profile_canonical).unwrap();
 let source=include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8.conduit");let search=include_bytes!("/home/dancxjo/conduit-4907-evaluation-identity/semantics/language/parser_window8_search.conduit");assert_eq!(hex(feature_contract()),"bdd64adf9c83923ed3e712f25a95670af25169c937da968c9b834bc1ae684181");
 let indices=StructuredInfoType::collection(StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),Some(25)).unwrap();let scores=StructuredInfoType::collection(StructuredInfoType::leaf(kind_id("value/i64")).unwrap(),Some(76)).unwrap();
 std::fs::write(output.join("numeric-indices.type.bin"),indices.canonical_bytes().unwrap()).unwrap();std::fs::write(output.join("numeric-scores.type.bin"),scores.canonical_bytes().unwrap()).unwrap();
 println!("signature={}",hex(signature.semantic_digest().unwrap()));println!("artifact_descriptor={}",hex(artifact.descriptor_digest(&signature).unwrap()));println!("reference={}",hex(artifact.content.semantic_digest().unwrap()));println!("lexical_declared={}",hex(semantic_digest("language/parser-declared-lexical@1",&profile_canonical)));
 for (name,digest) in [("feature",feature_contract()),("availability",semantic_digest("language/parser-window8-available@3",source)),("action",semantic_digest("language/parser-window8-action@3",source)),("joint_choice",semantic_digest("language/parser-window8-choice-search@3",search)),("numeric_indices",indices.semantic_digest().unwrap()),("numeric_scores",scores.semantic_digest().unwrap())]{println!("{name}={}",hex(digest));}
 println!("PASS independently fixed expected metadata only; no scorer/adoption/factory/Source execution or acceptance");
}

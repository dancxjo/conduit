#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
//! Actual Source projection and resource-admitted numerical Gear in one Plan/Play.
extern crate alloc;
pub use conduit_language::{
    LanguageId, LanguageLexicalCandidate, LanguageLexicalEntry, LanguageLexicalPos,
    LanguageLexicalProfile, LinguisticDerivationProvenance,
};
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
#[path = "common/parser_planned_runtime.rs"]
mod planned;
#[path = "../src/parser_model_selection.rs"]
mod selection;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn one_source_projection_and_admitted_model_plan_reuses_pinned_weights_for_three_frames() {
    let bytes = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
    let prepared = model_resource::categorical(
        bytes.to_vec(),
        selection::pinned_v2_model_signature().unwrap(),
        1,
    );
    let selected = selection::PreparedParserModelSelection::prepare(
        prepared.clone(),
        &selection::pinned_v2_lexical_profile().unwrap(),
    )
    .unwrap();
    assert_eq!(
        selected.expected_lexical_profile(),
        &selection::pinned_v2_lexical_profile().unwrap()
    );
    assert_eq!(selected.compatibility().lookups, 25);
    let mut execution = planned::prepare(selected.prepared_categorical().clone());
    drop(selected);
    drop(prepared);
    for frame in 0..3u64 {
        let indices: Vec<_> = (0..25u64).map(|i| (i * 13 + frame) % 413).collect();
        let basis = conduit_language::LanguageParserBasis::new(
            conduit_language::LanguageAnalysisRevisionId::new(format!("analysis/{frame}")).unwrap(),
            conduit_language::LanguageTextRevisionId::new(format!("source/{frame}")).unwrap(),
            conduit_language::LanguageTextId::new("utterance".into()).unwrap(),
        )
        .unwrap();
        let features = conduit_language::LanguageParserV2ModelFeatures::new(
            basis,
            indices.clone().try_into().unwrap(),
        )
        .unwrap();
        let value = features.into_structured().unwrap();
        let score_value = execution.infer(frame, &value);
        let StructuredInfoValueShape::Collection(scores) = score_value.shape() else {
            panic!("bare I64 scores")
        };
        assert_eq!(scores.len(), 76);
        for (class, score) in scores.iter().enumerate() {
            let StructuredInfoValueShape::Leaf(raw) = score.shape() else {
                panic!("I64 leaf")
            };
            let actual = i64::from_le_bytes(raw.try_into().unwrap());
            let expected: i64 = indices
                .iter()
                .map(|index| {
                    let offset = 20 + 2 * (class * 413 + *index as usize);
                    i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as i64
                })
                .sum();
            assert_eq!(actual, expected);
        }
    }
}

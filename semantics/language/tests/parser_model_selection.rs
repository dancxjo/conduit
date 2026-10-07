#![cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
use conduit_ai::*;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
use conduit_language::parser_model_selection::*;
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use model_resource::categorical;
use std::sync::Arc;
const BYTES: &[u8] = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");

#[test]
fn pinned_selection_retains_complete_model_profile_and_inspectable_compatibility() {
    let lexical = pinned_v2_lexical_profile().unwrap();
    let prepared = categorical(BYTES.to_vec(), pinned_v2_model_signature().unwrap(), 1);
    let selected = PreparedParserModelSelection::prepare(prepared.clone(), &lexical).unwrap();
    assert!(Arc::ptr_eq(selected.prepared_categorical(), &prepared));
    assert_eq!(selected.expected_lexical_profile(), &lexical);
    assert_eq!(selected.compatibility().maximum_score_magnitude, 128525);
    assert_eq!(selected.compatibility().lookups, 25);
    assert_eq!(selected.compatibility().scores, 76);
    assert_eq!(
        selected.compatibility().model_content,
        model_content_digest(BYTES)
    );
    assert_eq!(
        selected.compatibility().signature,
        pinned_v2_model_signature()
            .unwrap()
            .semantic_digest()
            .unwrap()
    );
    drop(prepared);
    assert_eq!(selected.prepared_categorical().resource().bytes(), BYTES);
}
#[test]
fn same_named_changed_lexical_material_and_foreign_signature_content_state_refuse_before_play() {
    let lexical = pinned_v2_lexical_profile().unwrap();
    let prepared = categorical(BYTES.to_vec(), pinned_v2_model_signature().unwrap(), 1);
    let first = lexical.entries().iter().next().unwrap();
    let changed = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
            first.surface().clone(),
            BoundedSequence::new(),
            LanguageLexicalPos::Other,
        )
        .unwrap()])
        .unwrap(),
        first.surface().clone(),
    )
    .unwrap();
    let entries =
        BoundedSequence::try_from_iter(lexical.entries().iter().enumerate().map(|(i, e)| {
            if i == 0 {
                changed.clone()
            } else {
                e.clone()
            }
        }))
        .unwrap();
    let forged = LanguageLexicalProfile::new(
        entries,
        lexical.identity().clone(),
        lexical.language().clone(),
        lexical.provenance().clone(),
    )
    .unwrap();
    assert_eq!(
        PreparedParserModelSelection::prepare(prepared, &forged).err(),
        Some(ParserModelSelectionRefusal::LexicalProfile)
    );
    let signature = pinned_v2_model_signature().unwrap();
    let foreign = ModelSignature::from_parts(
        "foreign/model-abi@1".into(),
        1,
        vec![ModelOperation::Infer],
        signature.inputs().get().as_slice().to_vec(),
        signature.outputs().get().as_slice().to_vec(),
    )
    .unwrap();
    assert_eq!(
        PreparedParserModelSelection::prepare(categorical(BYTES.to_vec(), foreign, 1), &lexical)
            .err(),
        Some(ParserModelSelectionRefusal::Signature)
    );
    let mut bytes = BYTES.to_vec();
    bytes[20] ^= 1;
    assert_eq!(
        PreparedParserModelSelection::prepare(categorical(bytes, signature.clone(), 1), &lexical)
            .err(),
        Some(ParserModelSelectionRefusal::ModelContent)
    );
    assert_eq!(
        PreparedParserModelSelection::prepare(categorical(BYTES.to_vec(), signature, 2), &lexical)
            .err(),
        Some(ParserModelSelectionRefusal::ModelStateSchema)
    );
}

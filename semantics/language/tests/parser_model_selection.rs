// The production module is deliberately exercised before its optional lib wiring.
extern crate alloc;
pub use conduit_language::{
    LanguageId, LanguageLexicalCandidate, LanguageLexicalEntry, LanguageLexicalPos,
    LanguageLexicalProfile, LinguisticDerivationProvenance,
};
#[path = "../src/parser_model_selection.rs"]
mod selection;
use conduit_ai::*;
#[path = "common/parser_model_resource.rs"]
mod model_resource;
use conduit_plot::rust_binding::BoundedSequence;
use model_resource::categorical;
use selection::*;
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

fn declaration_for(
    model: &conduit_ai::integer_categorical_step::PreparedCategoricalStep,
    identity: &str,
) -> ParserModelProfileDefinition {
    let pinned = PreparedParserModelSelection::prepare(
        categorical(BYTES.to_vec(), pinned_v2_model_signature().unwrap(), 1),
        &pinned_v2_lexical_profile().unwrap(),
    )
    .unwrap();
    let abi = pinned.compatibility();
    ParserModelProfileDefinition::new(
        identity.into(),
        1,
        pinned_v2_lexical_profile().unwrap(),
        LinguisticDerivationProvenance::model(
            "synthetic/admission-test-only".into(),
            "not-training-or-accuracy-evidence@1".into(),
        )
        .unwrap(),
        ParserSourceModelContract {
            feature_contract: abi.feature_contract,
            availability_contract: abi.availability_contract,
            action_contract: abi.action_contract,
            joint_choice_contract: abi.joint_choice_contract,
            numeric_indices_contract: model.indices_type().semantic_digest().unwrap(),
            numeric_scores_contract: model.scores_type().semantic_digest().unwrap(),
        },
        model.resource().artifact().clone(),
        model.resource().signature().clone(),
        model.dimensions(),
        V2_MAXIMUM_SCORE,
    )
    .unwrap()
}
#[test]
fn separately_declared_compatible_weights_retain_exact_custody_without_changing_pinned_profile() {
    let mut changed = BYTES.to_vec();
    changed[20] ^= 1;
    let alternate = categorical(changed.clone(), pinned_v2_model_signature().unwrap(), 1);
    let lexical = pinned_v2_lexical_profile().unwrap();
    assert_eq!(
        PreparedParserModelSelection::prepare(alternate.clone(), &lexical).err(),
        Some(ParserModelSelectionRefusal::ModelContent)
    );
    let definition = Arc::new(declaration_for(&alternate, "reviewed/test-alternative@1"));
    let source = definition.source_contract().clone();
    let selected = PreparedParserModelSelection::prepare_declared(
        alternate.clone(),
        definition.clone(),
        &lexical,
        &source,
    )
    .unwrap();
    let expected_provenance = definition.provenance().clone();
    drop(alternate);
    drop(definition);
    assert_eq!(
        selected.declaration().unwrap().provenance(),
        &expected_provenance
    );
    assert_eq!(selected.prepared_categorical().resource().bytes(), changed);
    assert_eq!(
        selected.declaration().unwrap().identity(),
        "reviewed/test-alternative@1"
    );
    assert_eq!(selected.declaration().unwrap().version(), 1);
    assert_eq!(
        selected.declaration().unwrap().artifact(),
        selected.prepared_categorical().resource().artifact()
    );
    assert_eq!(
        selected.compatibility().model_content,
        model_content_digest(&changed)
    );
}
#[test]
fn declared_selection_refuses_source_drift_and_another_admitted_artifact() {
    let original = categorical(BYTES.to_vec(), pinned_v2_model_signature().unwrap(), 1);
    let definition = Arc::new(declaration_for(&original, "reviewed/test-original@1"));
    let lexical = pinned_v2_lexical_profile().unwrap();
    let mut changed_source = definition.source_contract().clone();
    changed_source.feature_contract[0] ^= 1;
    assert_eq!(
        PreparedParserModelSelection::prepare_declared(
            original.clone(),
            definition.clone(),
            &lexical,
            &changed_source
        )
        .err(),
        Some(ParserModelSelectionRefusal::SourceContract)
    );
    let mut changed = BYTES.to_vec();
    changed[20] ^= 1;
    let other = categorical(changed, pinned_v2_model_signature().unwrap(), 1);
    assert_eq!(
        PreparedParserModelSelection::prepare_declared(
            other,
            definition.clone(),
            &lexical,
            definition.source_contract()
        )
        .err(),
        Some(ParserModelSelectionRefusal::ModelArtifact)
    );
    let mut incompatible = declaration_for(&original, "reviewed/test-bad-numeric@1");
    // The declaration is immutable to external callers; a separate declaration
    // with a different native port contract cannot admit this prepared owner.
    let mut source = incompatible.source_contract().clone();
    source.numeric_indices_contract[0] ^= 1;
    incompatible = ParserModelProfileDefinition::new(
        "reviewed/test-bad-numeric@1".into(),
        1,
        lexical.clone(),
        incompatible.provenance().clone(),
        source.clone(),
        original.resource().artifact().clone(),
        original.resource().signature().clone(),
        original.dimensions(),
        V2_MAXIMUM_SCORE,
    )
    .unwrap();
    assert_eq!(
        PreparedParserModelSelection::prepare_declared(
            original,
            Arc::new(incompatible),
            &lexical,
            &source
        )
        .err(),
        Some(ParserModelSelectionRefusal::SourceContract)
    );
}

#[test]
fn declared_dimensions_and_score_ceiling_cannot_expand_the_admitted_owner() {
    let model = categorical(BYTES.to_vec(), pinned_v2_model_signature().unwrap(), 1);
    let lexical = pinned_v2_lexical_profile().unwrap();
    let valid = declaration_for(&model, "reviewed/test-bounds@1");
    let make = |dimensions, maximum| {
        Arc::new(
            ParserModelProfileDefinition::new(
                "reviewed/test-bounds@1".into(),
                1,
                lexical.clone(),
                valid.provenance().clone(),
                valid.source_contract().clone(),
                model.resource().artifact().clone(),
                model.resource().signature().clone(),
                dimensions,
                maximum,
            )
            .unwrap(),
        )
    };
    let (features, scores, lookups) = model.dimensions();
    assert_eq!(
        PreparedParserModelSelection::prepare_declared(
            model.clone(),
            make((features + 1, scores, lookups), V2_MAXIMUM_SCORE),
            &lexical,
            valid.source_contract()
        )
        .err(),
        Some(ParserModelSelectionRefusal::ModelDimensions)
    );
    assert_eq!(
        PreparedParserModelSelection::prepare_declared(
            model.clone(),
            make(model.dimensions(), 0),
            &lexical,
            valid.source_contract()
        )
        .err(),
        Some(ParserModelSelectionRefusal::ScoreBound)
    );
    assert_eq!(
        ParserModelProfileDefinition::new(
            "reviewed/test-bounds@1".into(),
            0,
            lexical,
            valid.provenance().clone(),
            valid.source_contract().clone(),
            model.resource().artifact().clone(),
            model.resource().signature().clone(),
            model.dimensions(),
            V2_MAXIMUM_SCORE
        )
        .err(),
        Some(ParserModelSelectionRefusal::ProfileIdentity)
    );
}

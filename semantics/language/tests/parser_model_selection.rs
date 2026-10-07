// The production module is deliberately exercised before its optional lib wiring.
extern crate alloc;
pub use conduit_language::{
    LanguageId, LanguageLexicalCandidate, LanguageLexicalEntry, LanguageLexicalPos,
    LanguageLexicalProfile, LinguisticDerivationProvenance,
};
#[path = "../src/parser_model_selection.rs"]
mod selection;
use conduit_ai::{integer_categorical::*, integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
use conduit_plot::rust_binding::BoundedSequence;
use selection::*;
use std::sync::Arc;
const BYTES: &[u8] = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
fn categorical(
    bytes: Vec<u8>,
    signature: ModelSignature,
    state: u32,
) -> Arc<PreparedCategoricalStep> {
    let digest = model_content_digest(&bytes);
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest),
        content_profile: CATEGORICAL_I16_FORMAT.into(),
        access_class: "model/pinned-training-artifact/read@1".into(),
        extent: ResourceExtent {
            bytes: bytes.len() as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(digest),
            expires_at: None,
        },
    };
    let artifact = ModelArtifact {
        architecture_profile: CATEGORICAL_I16_ARCHITECTURE.into(),
        format_profile: CATEGORICAL_I16_FORMAT.into(),
        precision_profile: CATEGORICAL_I16_PRECISION.into(),
        state_schema_version: state,
        signature_identity: signature.semantic_digest().unwrap(),
        content: reference.clone(),
    };
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "model/test/read".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/exact-model".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    let model = Arc::new(
        AdmittedModelResource::adopt(artifact, signature, Arc::from(bytes), &binding).unwrap(),
    );
    let residence = ResourceContentOffer {
        contract: ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile,
            maximum_bytes: reference.extent.bytes as u32,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 16,
            publication_slots: 0,
            sensitive: false,
        },
        owner_host: "test/model-host".into(),
        owner_boot: "test/model-boot".into(),
        base_id: "test/model-base".into(),
        residence_profile: "test/immutable-memory".into(),
    };
    Arc::new(PreparedCategoricalStep::prepare(model, "test/model-pool".into(), residence).unwrap())
}
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

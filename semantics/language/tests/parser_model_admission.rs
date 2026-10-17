#![cfg(all(feature = "parser-model", target_has_atomic = "ptr"))]
use conduit_ai::{AdmittedModelResource, ModelArtifact, ModelSignature, MODEL_READ_AUTHORITY};
use conduit_core::*;
use conduit_language::parser_model::*;
use std::sync::Arc;

fn adopt(
    artifact: ModelArtifact,
    signature: ModelSignature,
    bytes: &[u8],
) -> Arc<AdmittedModelResource> {
    let reference = &artifact.content;
    let binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: "test/parser-model-handle".into(),
        authority_contract: MODEL_READ_AUTHORITY.into(),
        authority_grant: "test/parser-model-read".into(),
        maximum_bytes: reference.extent.bytes,
        maximum_items: Some(1),
        availability: ResourceReferenceAvailability::Available,
    };
    Arc::new(AdmittedModelResource::adopt(artifact, signature, Arc::from(bytes), &binding).unwrap())
}
fn residence() -> ResourceContentOffer {
    let (artifact, _) = english_parser_scorer_descriptor();
    let reference = artifact.content;
    ResourceContentOffer {
        contract: ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile,
            maximum_bytes: ENGLISH_PARSER_MODEL_BYTES as u32,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: 16,
            publication_slots: 0,
            sensitive: false,
        },
        owner_host: "parser-model-test".into(),
        owner_boot: "parser-model-test-boot".into(),
        base_id: "parser-model-test-base".into(),
        residence_profile: "test/owned-immutable-memory@1".into(),
    }
}
fn selected(
    resource: Option<Arc<AdmittedModelResource>>,
    offer: ResourceContentOffer,
) -> Result<conduit_ai::integer_categorical_step::PreparedCategoricalStep, ParserScorerRefusal> {
    prepare_english_parser_scorer(resource, "test/parser-model-pool".into(), offer)
}
fn pinned() -> Arc<AdmittedModelResource> {
    let (artifact, signature) = english_parser_scorer_descriptor();
    adopt(artifact, signature, ENGLISH_PARSER_WEIGHTS)
}
#[test]
fn real_adopted_pinned_resource_prepares_existing_generic_step() {
    let model = pinned();
    let step = selected(Some(model.clone()), residence()).unwrap();
    assert_eq!(step.dimensions(), ENGLISH_PARSER_DIMENSIONS);
    assert_eq!(
        step.resource().descriptor_identity(),
        model.descriptor_identity()
    );
    assert_eq!(step.resource().bytes(), ENGLISH_PARSER_WEIGHTS);
    assert_eq!(
        step.resource_offer()
            .content
            .as_ref()
            .unwrap()
            .contract
            .maximum_bytes,
        62_796
    );
}
#[test]
fn absence_and_foreign_model_signature_and_descriptor_have_named_refusals() {
    assert!(matches!(
        selected(None, residence()),
        Err(ParserScorerRefusal::Absent)
    ));
    let (mut artifact, signature) = english_parser_scorer_descriptor();
    let mut changed = ENGLISH_PARSER_WEIGHTS.to_vec();
    changed[20] ^= 1;
    let digest = conduit_ai::model_content_digest(&changed);
    artifact.content.identity = ResourceSemanticIdentity::from_digest(digest);
    artifact.content.lifetime.version = ResourceVersionIdentity::from_digest(digest);
    let foreign = adopt(artifact, signature, &changed);
    assert!(matches!(
        selected(Some(foreign), residence()),
        Err(ParserScorerRefusal::ForeignModel)
    ));

    let (mut artifact, mut signature) = english_parser_scorer_descriptor();
    signature.compatibility_version += 1;
    artifact.signature_identity = signature.semantic_digest().unwrap();
    let foreign = adopt(artifact, signature, ENGLISH_PARSER_WEIGHTS);
    assert!(matches!(
        selected(Some(foreign), residence()),
        Err(ParserScorerRefusal::ForeignSignature)
    ));

    let (mut artifact, signature) = english_parser_scorer_descriptor();
    artifact.content.lifetime.version = ResourceVersionIdentity::from_digest([9; 32]);
    let foreign = adopt(artifact, signature, ENGLISH_PARSER_WEIGHTS);
    assert!(matches!(
        selected(Some(foreign), residence()),
        Err(ParserScorerRefusal::ForeignDescriptor)
    ));

    let (mut artifact, signature) = english_parser_scorer_descriptor();
    artifact.content.access_class = "test/foreign-model/read@1".into();
    let foreign = adopt(artifact, signature, ENGLISH_PARSER_WEIGHTS);
    assert!(matches!(
        selected(Some(foreign), residence()),
        Err(ParserScorerRefusal::ForeignDescriptor)
    ));
}
#[test]
fn wrong_resource_identity_extent_and_capacity_refuse_selection() {
    let mut offer = residence();
    offer.contract.maximum_bytes -= 1;
    assert!(matches!(
        selected(Some(pinned()), offer),
        Err(ParserScorerRefusal::ResourceBounds)
    ));
    let mut offer = residence();
    offer.contract.maximum_bytes += 1;
    assert!(matches!(
        selected(Some(pinned()), offer),
        Err(ParserScorerRefusal::ResourceBounds)
    ));
    let mut offer = residence();
    offer.contract.reader_leases = 0;
    assert!(matches!(
        selected(Some(pinned()), offer),
        Err(ParserScorerRefusal::ResourceBounds)
    ));
    let mut offer = residence();
    offer.contract.identity = ResourceSemanticIdentity::from_digest([7; 32]);
    assert!(matches!(
        selected(Some(pinned()), offer),
        Err(ParserScorerRefusal::ResourceBounds)
    ));
    let mut offer = residence();
    offer.contract.maximum_items = 2;
    assert!(matches!(
        selected(Some(pinned()), offer),
        Err(ParserScorerRefusal::ResourceBounds)
    ));
}

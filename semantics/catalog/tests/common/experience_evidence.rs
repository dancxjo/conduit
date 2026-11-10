use conduit_human::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};

pub fn identity(value: &str) -> ExperienceEvidenceIdentity {
    ExperienceEvidenceIdentity::new(value.into()).unwrap()
}

pub fn selection(ids: &[&str]) -> ExperienceEvidenceSelection {
    ExperienceEvidenceSelection::new(
        ExperienceEvidenceIdentities::new(
            BoundedSequence::try_from_iter(ids.iter().map(|id| identity(id))).unwrap(),
        )
        .unwrap(),
        identity("provenance/7"),
        7,
    )
    .unwrap()
}

pub fn item(
    id: &str,
    origin: ExperienceOrigin,
    role: ExperienceTemporalRole,
    availability: ExperienceAvailability,
    revision: u64,
    provenance: &str,
    content: &[u8],
) -> ExperienceEvidenceItem {
    ExperienceEvidenceItem::new(
        availability,
        ExperienceCertainty::Uncertain,
        identity("value/text"),
        ExperienceDomain::HumanUtterance,
        ExperienceEvidenceContent::new(BoundedBytes::new(content).unwrap()).unwrap(),
        identity(id),
        origin,
        identity(provenance),
        revision,
        ExperienceEvidenceSources::new(
            BoundedSequence::try_from_iter(vec![ExperienceEvidenceSource::source(identity(
                "source/utterance",
            ))
            .unwrap()])
            .unwrap(),
        )
        .unwrap(),
        role,
    )
    .unwrap()
}

pub fn observation(id: &str) -> ExperienceEvidenceItem {
    item(
        id,
        ExperienceOrigin::Observation,
        ExperienceTemporalRole::Current,
        ExperienceAvailability::Present,
        7,
        "provenance/7",
        b"observed evidence",
    )
}

pub fn with_sources(
    item: ExperienceEvidenceItem,
    sources: Vec<ExperienceEvidenceSource>,
) -> ExperienceEvidenceItem {
    ExperienceEvidenceItem::new(
        *item.availability(),
        *item.certainty(),
        item.content_kind().clone(),
        *item.domain(),
        item.encoded_content().clone(),
        item.identity().clone(),
        *item.origin(),
        item.provenance_revision().clone(),
        *item.revision(),
        ExperienceEvidenceSources::new(BoundedSequence::try_from_iter(sources).unwrap()).unwrap(),
        *item.temporal_role(),
    )
    .unwrap()
}

pub fn resource_source() -> ExperienceEvidenceSource {
    use conduit_core::*;
    ExperienceEvidenceSource::resource(BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([3; 32]),
        content_profile: kind_id("value/text"),
        access_class: ResourceClassId::from("conduit.resource/portable-content@1"),
        extent: ResourceExtent {
            bytes: 42,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([7; 32]),
            expires_at: None,
        },
    })
    .unwrap()
}

pub fn items(values: Vec<ExperienceEvidenceItem>) -> ExperienceEvidenceItems {
    ExperienceEvidenceItems::new(BoundedSequence::try_from_iter(values).unwrap()).unwrap()
}

pub fn request(ids: &[&str], values: Vec<ExperienceEvidenceItem>) -> ExperienceEvidenceRequest {
    ExperienceEvidenceRequest::new(items(values), selection(ids)).unwrap()
}

#[allow(dead_code)]
pub fn encode<T: NativeRustBinding>(value: T) -> Vec<u8> {
    value.into_structured().unwrap().canonical_bytes().unwrap()
}

//! Frozen d130 canonical transcript traversal, solely an independent diagnostic oracle.
//! Production validation never calls this module. Hash parity is required before export.
use crate::*;
use alloc::vec::Vec;
use sha2::Digest;
struct Sha256 {
    inner: sha2::Sha256,
    chunks: Vec<Vec<u8>>,
}
impl Sha256 {
    fn new() -> Self {
        Self {
            inner: sha2::Sha256::new(),
            chunks: Vec::new(),
        }
    }
    fn update(&mut self, bytes: impl AsRef<[u8]>) {
        self.inner.update(bytes.as_ref());
        self.chunks.push(bytes.as_ref().to_vec());
    }
    fn finish(self) -> ([u8; 32], Vec<Vec<u8>>) {
        (self.inner.finalize().into(), self.chunks)
    }
}
pub(super) fn capture(face: &Face) -> ([u8; 32], Vec<Vec<u8>>) {
    face.frozen_content_digest()
}

impl Face {
    fn frozen_content_digest(&self) -> ([u8; 32], Vec<Vec<u8>>) {
        let mut digest = Sha256::new();
        hash_string(&mut digest, "conduit.face/face@1");
        digest.update(self.revision.to_le_bytes());
        hash_string(&mut digest, &self.interaction_context.identity);
        for statement in &self.interaction_context.basis {
            hash_string(&mut digest, &statement.source);
            hash_relationship(&mut digest, &statement.relationship);
            hash_string(&mut digest, &statement.target);
        }
        hash_string(
            &mut digest,
            self.basis.body_id.as_ref().map_or("", |id| id.as_str()),
        );
        hash_string(
            &mut digest,
            self.basis.wake_id.as_ref().map_or("", |id| id.as_str()),
        );
        hash_string(
            &mut digest,
            self.basis
                .source_document_id
                .as_ref()
                .map_or("", |id| id.as_str()),
        );
        hash_string(
            &mut digest,
            self.basis
                .checked_plot_id
                .as_ref()
                .map_or("", |id| id.as_str()),
        );
        hash_optional(
            &mut digest,
            self.basis.expanded_plot_id.as_ref().map(|id| id.as_str()),
        );
        hash_optional(
            &mut digest,
            self.basis.plan_id.as_ref().map(|id| id.as_str()),
        );
        hash_optional(
            &mut digest,
            self.basis.active_play_id.as_ref().map(|id| id.as_str()),
        );
        for sign in &self.basis.sign_ids {
            hash_string(&mut digest, sign.as_str());
        }
        for subject in &self.subjects {
            hash_string(&mut digest, &subject.identity);
            hash_role(&mut digest, &subject.role);
            hash_string(&mut digest, &subject.name);
        }
        for relationship in &self.relationships {
            hash_string(&mut digest, &relationship.source);
            hash_string(&mut digest, &relationship.target);
            hash_relationship(&mut digest, &relationship.kind);
        }
        self.frozen_hash_rhetorical_composition(&mut digest);
        for property in &self.properties {
            hash_string(&mut digest, &property.subject);
            hash_string(&mut digest, &property.name);
            match &property.value {
                FacePropertyValue::Identity(value) => {
                    digest.update([0]);
                    hash_string(&mut digest, value);
                }
                FacePropertyValue::BaseImplementationId(base) => {
                    digest.update([1]);
                    hash_string(&mut digest, base.as_str());
                }
                FacePropertyValue::Text(value) => {
                    digest.update([2]);
                    hash_string(&mut digest, value);
                }
                FacePropertyValue::Count(value) => {
                    digest.update([3]);
                    digest.update(value.to_le_bytes());
                }
                FacePropertyValue::Signed(value) => {
                    digest.update([5]);
                    digest.update(value.to_le_bytes());
                }
                FacePropertyValue::Flag(value) => {
                    digest.update([4, u8::from(*value)]);
                }
                FacePropertyValue::Content(encoded) => {
                    digest.update([6]);
                    digest.update((encoded.len() as u32).to_le_bytes());
                    digest.update(encoded);
                }
                FacePropertyValue::TypedValue { contract, bytes } => {
                    digest.update([8]);
                    hash_contract(&mut digest, contract);
                    digest.update((bytes.len() as u32).to_le_bytes());
                    digest.update(bytes);
                }
                FacePropertyValue::ValueContract(contract) => {
                    digest.update([7]);
                    hash_contract(&mut digest, contract);
                }
            }
        }
        for item in &self.text {
            hash_string(&mut digest, &item.subject);
            hash_string(&mut digest, &item.text);
        }
        self.frozen_hash_semantics(&mut digest);
        self.frozen_hash_temporal(&mut digest);
        digest.finish()
    }
    fn frozen_hash_semantics(&self, digest: &mut Sha256) {
        for action in &self.actions {
            hash_string(digest, &action.identity);
            hash_string(digest, &action.intent);
            hash_string(digest, &action.target);
            hash_string(digest, &action.name);
            digest.update((action.arguments.len() as u32).to_le_bytes());
            for argument in &action.arguments {
                hash_string(digest, &argument.name);
                hash_string(digest, &argument.value_name);
                hash_contract(digest, &argument.contract);
            }
            digest.update([action.disclosure as u8]);
            match &action.availability {
                FaceActionAvailability::Available => digest.update([0]),
                FaceActionAvailability::Unavailable {
                    reason_code,
                    explanation,
                } => {
                    digest.update([1]);
                    hash_string(digest, reason_code);
                    hash_string(digest, explanation);
                }
                FaceActionAvailability::Refused {
                    reason_code,
                    explanation,
                } => {
                    digest.update([2]);
                    hash_string(digest, reason_code);
                    hash_string(digest, explanation);
                }
            }
        }
        for disclosure in &self.disclosures {
            hash_string(digest, &disclosure.subject);
            digest.update([disclosure.level as u8]);
        }
    }
    fn frozen_hash_temporal(&self, digest: &mut Sha256) {
        for reference in &self.temporal_references {
            hash_string(digest, &reference.identity);
            hash_instant(digest, &reference.instant);
        }
        for fact in &self.temporal_facts {
            hash_string(digest, &fact.subject);
            digest.update([fact.role as u8]);
            digest.update([u8::from(fact.sign_id.is_some())]);
            if let Some(sign_id) = &fact.sign_id {
                hash_string(digest, sign_id.as_str());
            }
            hash_instant(digest, &fact.source);
            hash_string(digest, &fact.reference);
            hash_relation(digest, fact.relation);
        }
    }
    fn frozen_hash_rhetorical_composition(&self, digest: &mut Sha256) {
        for relation in &self.composition {
            hash_string(digest, &relation.identity);
            hash_string(digest, &relation.source);
            hash_string(digest, &relation.target);
            match &relation.kind {
                FaceCompositionKind::Group => digest.update([0]),
                FaceCompositionKind::Contrast => digest.update([1]),
                FaceCompositionKind::Juxtapose => digest.update([2]),
                FaceCompositionKind::Emphasize => digest.update([3]),
                FaceCompositionKind::Subordinate => digest.update([4]),
                FaceCompositionKind::Associate => digest.update([5]),
                FaceCompositionKind::RevealAfter => digest.update([6]),
                FaceCompositionKind::Semantic(payload) => {
                    digest.update([u8::MAX]);
                    hash_string(digest, payload.identity());
                }
            }
        }
    }
}
fn hash_role(digest: &mut Sha256, role: &FaceRole) {
    match role {
        FaceRole::Semantic(identity) => {
            digest.update([u8::MAX]);
            hash_string(digest, identity.as_str());
        }
        broad => digest.update([broad_role_tag(broad)]),
    }
}
fn broad_role_tag(role: &FaceRole) -> u8 {
    match role {
        FaceRole::Document => 0,
        FaceRole::Body => 1,
        FaceRole::Part => 2,
        FaceRole::Candidate => 3,
        FaceRole::Plot => 4,
        FaceRole::Gear => 5,
        FaceRole::Port => 6,
        FaceRole::Cord => 7,
        FaceRole::Plan => 8,
        FaceRole::Play => 9,
        FaceRole::Host => 10,
        FaceRole::Capability => 11,
        FaceRole::Line => 12,
        FaceRole::Manifestation => 13,
        FaceRole::Route => 14,
        FaceRole::Diagnostic => 15,
        FaceRole::Sign => 16,
        FaceRole::Info => 17,
        FaceRole::Region => 18,
        FaceRole::Collection => 19,
        FaceRole::Item => 20,
        FaceRole::TextEntry => 21,
        FaceRole::Status => 22,
        FaceRole::Action => 23,
        FaceRole::Semantic(_) => unreachable!("semantic roles are hashed separately"),
    }
}
fn hash_relationship(digest: &mut Sha256, relationship: &FaceRelationshipKind) {
    match relationship {
        FaceRelationshipKind::Contains => digest.update([0]),
        FaceRelationshipKind::Connects => digest.update([1]),
        FaceRelationshipKind::Describes => digest.update([2]),
        FaceRelationshipKind::Realizes => digest.update([3]),
        FaceRelationshipKind::Observes => digest.update([4]),
        FaceRelationshipKind::Semantic(identity) => {
            digest.update([u8::MAX]);
            hash_string(digest, identity.as_str());
        }
    }
}
fn hash_string(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u32).to_le_bytes());
    digest.update(value.as_bytes());
}
fn hash_optional(digest: &mut Sha256, value: Option<&str>) {
    digest.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        hash_string(digest, value);
    }
}
fn hash_contract(digest: &mut Sha256, contract: &conduit_core::CheckedValueContract) {
    digest.update((contract.identity_byte_len() as u32).to_le_bytes());
    contract.visit_identity_bytes(|bytes| digest.update(bytes));
}
fn hash_instant(digest: &mut Sha256, instant: &TemporalInstant) {
    digest.update(instant.ticks.to_le_bytes());
    digest.update([instant.scale as u8]);
    hash_string(digest, &instant.clock_basis);
    digest.update(instant.resolution_ticks.to_le_bytes());
    digest.update(instant.uncertainty_ticks.to_le_bytes());
}
fn hash_relation(digest: &mut Sha256, relation: TemporalRelation) {
    match relation {
        TemporalRelation::Past {
            minimum_ticks,
            maximum_ticks,
        } => {
            digest.update([0]);
            digest.update(minimum_ticks.to_le_bytes());
            digest.update(maximum_ticks.to_le_bytes());
        }
        TemporalRelation::Present => digest.update([1]),
        TemporalRelation::Future {
            minimum_ticks,
            maximum_ticks,
        } => {
            digest.update([2]);
            digest.update(minimum_ticks.to_le_bytes());
            digest.update(maximum_ticks.to_le_bytes());
        }
        TemporalRelation::Indeterminate => digest.update([3]),
    }
}

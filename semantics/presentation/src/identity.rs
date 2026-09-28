//! Deterministic identity for one exact portable Presentation revision.

use alloc::string::String;
use sha2::{Digest, Sha256};

use crate::{
    Presentation, PresentationPropertyValue, PresentationRelationshipKind, PresentationRole,
};

impl Presentation {
    pub(crate) fn content_digest(&self) -> String {
        let mut digest = Sha256::new();
        hash_string(&mut digest, "conduit.presentation/presentation@1");
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
                .checked_form_id
                .as_ref()
                .map_or("", |id| id.as_str()),
        );
        hash_optional(
            &mut digest,
            self.basis.expanded_form_id.as_ref().map(|id| id.as_str()),
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
        self.hash_rhetorical_composition(&mut digest);
        for property in &self.properties {
            hash_string(&mut digest, &property.subject);
            hash_string(&mut digest, &property.name);
            match &property.value {
                PresentationPropertyValue::Identity(value) => {
                    digest.update([0]);
                    hash_string(&mut digest, value);
                }
                PresentationPropertyValue::BaseImplementationId(base) => {
                    digest.update([1]);
                    hash_string(&mut digest, base.as_str());
                }
                PresentationPropertyValue::Text(value) => {
                    digest.update([2]);
                    hash_string(&mut digest, value);
                }
                PresentationPropertyValue::Count(value) => {
                    digest.update([3]);
                    digest.update(value.to_le_bytes());
                }
                PresentationPropertyValue::Signed(value) => {
                    digest.update([5]);
                    digest.update(value.to_le_bytes());
                }
                PresentationPropertyValue::Flag(value) => {
                    digest.update([4, u8::from(*value)]);
                }
                PresentationPropertyValue::Content(encoded) => {
                    digest.update([6]);
                    digest.update((encoded.len() as u32).to_le_bytes());
                    digest.update(encoded);
                }
                PresentationPropertyValue::ValueContract(contract) => {
                    digest.update([7]);
                    let identity = contract.identity_bytes();
                    digest.update((identity.len() as u32).to_le_bytes());
                    digest.update(identity);
                }
            }
        }
        for item in &self.text {
            hash_string(&mut digest, &item.subject);
            hash_string(&mut digest, &item.text);
        }
        self.hash_semantics(&mut digest);
        self.hash_temporal(&mut digest);
        let bytes: [u8; 32] = digest.finalize().into();
        hex(&bytes)
    }
}

fn hash_role(digest: &mut Sha256, role: &PresentationRole) {
    match role {
        PresentationRole::Semantic(identity) => {
            digest.update([u8::MAX]);
            hash_string(digest, identity.as_str());
        }
        broad => digest.update([broad_role_tag(broad)]),
    }
}

fn broad_role_tag(role: &PresentationRole) -> u8 {
    match role {
        PresentationRole::Document => 0,
        PresentationRole::Body => 1,
        PresentationRole::Part => 2,
        PresentationRole::Candidate => 3,
        PresentationRole::Form => 4,
        PresentationRole::Gear => 5,
        PresentationRole::Port => 6,
        PresentationRole::Cord => 7,
        PresentationRole::Plan => 8,
        PresentationRole::Play => 9,
        PresentationRole::Host => 10,
        PresentationRole::Capability => 11,
        PresentationRole::Line => 12,
        PresentationRole::Manifestation => 13,
        PresentationRole::Route => 14,
        PresentationRole::Diagnostic => 15,
        PresentationRole::Sign => 16,
        PresentationRole::Info => 17,
        PresentationRole::Region => 18,
        PresentationRole::Collection => 19,
        PresentationRole::Item => 20,
        PresentationRole::TextEntry => 21,
        PresentationRole::Status => 22,
        PresentationRole::Action => 23,
        PresentationRole::Semantic(_) => unreachable!("semantic roles are hashed separately"),
    }
}

pub(crate) fn hash_relationship(digest: &mut Sha256, relationship: &PresentationRelationshipKind) {
    match relationship {
        PresentationRelationshipKind::Contains => digest.update([0]),
        PresentationRelationshipKind::Connects => digest.update([1]),
        PresentationRelationshipKind::Describes => digest.update([2]),
        PresentationRelationshipKind::Realizes => digest.update([3]),
        PresentationRelationshipKind::Observes => digest.update([4]),
        PresentationRelationshipKind::Semantic(identity) => {
            digest.update([u8::MAX]);
            hash_string(digest, identity.as_str());
        }
    }
}

pub(crate) fn hash_string(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u32).to_le_bytes());
    digest.update(value.as_bytes());
}

fn hash_optional(digest: &mut Sha256, value: Option<&str>) {
    digest.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        hash_string(digest, value);
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

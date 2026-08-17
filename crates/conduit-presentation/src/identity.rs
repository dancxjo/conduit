//! Deterministic identity for one exact portable Presentation revision.

use alloc::string::String;
use sha2::{Digest, Sha256};

use crate::{Presentation, PresentationPropertyValue};

impl Presentation {
    pub(crate) fn content_digest(&self) -> String {
        let mut digest = Sha256::new();
        hash_string(&mut digest, "conduit.presentation/presentation@1");
        digest.update(self.revision.to_le_bytes());
        // Preserve established embodied Presentation identities: Some
        // values hash exactly as the formerly required fields did. None
        // uses the otherwise-invalid empty identity as the zero-Body marker.
        hash_string(
            &mut digest,
            self.basis.seed_id.as_ref().map_or("", |id| id.as_str()),
        );
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
            digest.update([subject.role as u8]);
            hash_string(&mut digest, &subject.label);
            hash_string(&mut digest, &subject.accessibility_name);
        }
        for relationship in &self.relationships {
            hash_string(&mut digest, &relationship.source);
            hash_string(&mut digest, &relationship.target);
            digest.update([relationship.kind as u8]);
        }
        for property in &self.properties {
            hash_string(&mut digest, &property.subject);
            hash_string(&mut digest, &property.name);
            match &property.value {
                PresentationPropertyValue::Identity(value) => {
                    digest.update([0]);
                    hash_string(&mut digest, value);
                }
                PresentationPropertyValue::ConnectionBase(base) => {
                    digest.update([1, base.canonical_code()]);
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

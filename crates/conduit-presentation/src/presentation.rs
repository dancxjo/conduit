//! Bounded, implementation-neutral content to be made perceptible.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    ActivePlayId, CheckedFormId, EvidenceId, ExpandedFormId, PlanId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_PRESENTATION_SUBJECTS: usize = 1_024;
pub const MAX_PRESENTATION_RELATIONSHIPS: usize = 2_048;
pub const MAX_PRESENTATION_TEXT_ITEMS: usize = 2_048;
pub const MAX_PRESENTATION_EVIDENCE: usize = 1_024;
pub const MAX_PRESENTATION_ID_BYTES: usize = 256;
pub const MAX_PRESENTATION_TEXT_BYTES: usize = 1_024;
pub const MAX_PRESENTATION_TOTAL_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PresentationContentId(String);

impl PresentationContentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationBasis {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: Option<ExpandedFormId>,
    pub plan_id: Option<PlanId>,
    pub active_play_id: Option<ActivePlayId>,
    pub evidence_ids: Vec<EvidenceId>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationRole {
    Document,
    Form,
    Cell,
    Port,
    Cord,
    Plan,
    Play,
    Host,
    Route,
    Diagnostic,
    Evidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationSubject {
    pub identity: String,
    pub role: PresentationRole,
    pub label: String,
    pub accessibility_name: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationRelationshipKind {
    Contains,
    Connects,
    Describes,
    Realizes,
    Observes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationRelationship {
    pub source: String,
    pub target: String,
    pub kind: PresentationRelationshipKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationText {
    pub subject: String,
    pub text: String,
}

/// One immutable semantic presentation revision.
///
/// Geometry, viewport, toolkit objects, window handles, DOM identity, pixel
/// storage, and provider resources are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    pub identity: PresentationContentId,
    pub revision: u64,
    pub basis: PresentationBasis,
    pub subjects: Vec<PresentationSubject>,
    pub relationships: Vec<PresentationRelationship>,
    pub text: Vec<PresentationText>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationError {
    EmptySubjects,
    TooManySubjects,
    TooManyRelationships,
    TooManyTextItems,
    TooMuchEvidence,
    EmptyIdentity,
    IdentityTooLong,
    EmptyText,
    TextTooLong,
    DuplicateSubject,
    UnknownRelationshipSubject,
    UnknownTextSubject,
    DuplicateEvidence,
    NonCanonicalEvidence,
    InvalidBasis,
    TooManyBytes,
    InvalidIdentity,
}

impl core::fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid portable Presentation: {self:?}")
    }
}

impl Presentation {
    pub fn new(
        revision: u64,
        mut basis: PresentationBasis,
        subjects: Vec<PresentationSubject>,
        relationships: Vec<PresentationRelationship>,
        text: Vec<PresentationText>,
    ) -> Result<Self, PresentationError> {
        basis.evidence_ids.sort();
        if basis.evidence_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(PresentationError::DuplicateEvidence);
        }
        let mut value = Self {
            identity: PresentationContentId(String::new()),
            revision,
            basis,
            subjects,
            relationships,
            text,
        };
        value.validate_content()?;
        value.identity = PresentationContentId(value.content_digest());
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), PresentationError> {
        self.validate_content()?;
        if self.identity.0 != self.content_digest() {
            return Err(PresentationError::InvalidIdentity);
        }
        Ok(())
    }

    fn validate_content(&self) -> Result<(), PresentationError> {
        if self.subjects.is_empty() {
            return Err(PresentationError::EmptySubjects);
        }
        if self.subjects.len() > MAX_PRESENTATION_SUBJECTS {
            return Err(PresentationError::TooManySubjects);
        }
        if self.relationships.len() > MAX_PRESENTATION_RELATIONSHIPS {
            return Err(PresentationError::TooManyRelationships);
        }
        if self.text.len() > MAX_PRESENTATION_TEXT_ITEMS {
            return Err(PresentationError::TooManyTextItems);
        }
        if self.basis.evidence_ids.len() > MAX_PRESENTATION_EVIDENCE {
            return Err(PresentationError::TooMuchEvidence);
        }
        if self
            .basis
            .evidence_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(PresentationError::NonCanonicalEvidence);
        }
        validate_id(self.basis.source_document_id.as_str())?;
        validate_id(self.basis.checked_form_id.as_str())?;
        for identity in [
            self.basis
                .expanded_form_id
                .as_ref()
                .map(|value| value.as_str()),
            self.basis.plan_id.as_ref().map(|value| value.as_str()),
            self.basis
                .active_play_id
                .as_ref()
                .map(|value| value.as_str()),
        ]
        .into_iter()
        .flatten()
        {
            validate_id(identity)?;
        }
        if self.basis.plan_id.is_some() != self.basis.expanded_form_id.is_some()
            || (self.basis.active_play_id.is_some() && self.basis.plan_id.is_none())
        {
            return Err(PresentationError::InvalidBasis);
        }
        for evidence in &self.basis.evidence_ids {
            validate_id(evidence.as_str())?;
        }
        for subject in &self.subjects {
            validate_id(&subject.identity)?;
            validate_text(&subject.label)?;
            validate_text(&subject.accessibility_name)?;
        }
        for index in 0..self.subjects.len() {
            if self.subjects[index + 1..]
                .iter()
                .any(|subject| subject.identity == self.subjects[index].identity)
            {
                return Err(PresentationError::DuplicateSubject);
            }
        }
        for relationship in &self.relationships {
            if !self.has_subject(&relationship.source) || !self.has_subject(&relationship.target) {
                return Err(PresentationError::UnknownRelationshipSubject);
            }
        }
        for item in &self.text {
            if !self.has_subject(&item.subject) {
                return Err(PresentationError::UnknownTextSubject);
            }
            validate_text(&item.text)?;
        }
        let total_bytes = self
            .basis
            .source_document_id
            .as_str()
            .len()
            .saturating_add(self.basis.checked_form_id.as_str().len())
            .saturating_add(optional_len(
                self.basis.expanded_form_id.as_ref().map(|id| id.as_str()),
            ))
            .saturating_add(optional_len(
                self.basis.plan_id.as_ref().map(|id| id.as_str()),
            ))
            .saturating_add(optional_len(
                self.basis.active_play_id.as_ref().map(|id| id.as_str()),
            ))
            .saturating_add(
                self.basis
                    .evidence_ids
                    .iter()
                    .map(|id| id.as_str().len())
                    .sum::<usize>(),
            )
            .saturating_add(
                self.subjects
                    .iter()
                    .map(|subject| {
                        subject.identity.len()
                            + subject.label.len()
                            + subject.accessibility_name.len()
                            + 1
                    })
                    .sum::<usize>(),
            )
            .saturating_add(
                self.relationships
                    .iter()
                    .map(|relationship| relationship.source.len() + relationship.target.len() + 1)
                    .sum::<usize>(),
            )
            .saturating_add(
                self.text
                    .iter()
                    .map(|item| item.subject.len() + item.text.len())
                    .sum::<usize>(),
            );
        if total_bytes > MAX_PRESENTATION_TOTAL_BYTES {
            return Err(PresentationError::TooManyBytes);
        }
        Ok(())
    }

    fn has_subject(&self, identity: &str) -> bool {
        self.subjects
            .iter()
            .any(|subject| subject.identity == identity)
    }

    fn content_digest(&self) -> String {
        let mut digest = Sha256::new();
        hash_string(&mut digest, "conduit.presentation/presentation@1");
        digest.update(self.revision.to_le_bytes());
        hash_string(&mut digest, self.basis.source_document_id.as_str());
        hash_string(&mut digest, self.basis.checked_form_id.as_str());
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
        for evidence in &self.basis.evidence_ids {
            hash_string(&mut digest, evidence.as_str());
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
        for item in &self.text {
            hash_string(&mut digest, &item.subject);
            hash_string(&mut digest, &item.text);
        }
        let bytes: [u8; 32] = digest.finalize().into();
        hex(&bytes)
    }
}

fn validate_id(value: &str) -> Result<(), PresentationError> {
    if value.is_empty() {
        Err(PresentationError::EmptyIdentity)
    } else if value.len() > MAX_PRESENTATION_ID_BYTES {
        Err(PresentationError::IdentityTooLong)
    } else {
        Ok(())
    }
}

fn validate_text(value: &str) -> Result<(), PresentationError> {
    if value.is_empty() {
        Err(PresentationError::EmptyText)
    } else if value.len() > MAX_PRESENTATION_TEXT_BYTES {
        Err(PresentationError::TextTooLong)
    } else {
        Ok(())
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

fn optional_len(value: Option<&str>) -> usize {
    value.map_or(0, str::len)
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

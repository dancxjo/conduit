//! Bounded, implementation-neutral content to be made perceptible.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_body::{BodyId, SeedId, WakeId};
use conduit_core::{
    ActivePlayId, CheckedFormId, ConnectionBase, ExpandedFormId, PlanId, SignId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_PRESENTATION_SUBJECTS: usize = 1_024;
pub const MAX_PRESENTATION_RELATIONSHIPS: usize = 2_048;
pub const MAX_PRESENTATION_TEXT_ITEMS: usize = 2_048;
pub const MAX_PRESENTATION_PROPERTIES: usize = 4_096;
pub const MAX_PRESENTATION_SIGNS: usize = 1_024;
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
    pub seed_id: SeedId,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: Option<ExpandedFormId>,
    pub plan_id: Option<PlanId>,
    pub active_play_id: Option<ActivePlayId>,
    pub sign_ids: Vec<SignId>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationRole {
    Document,
    Body,
    Part,
    Candidate,
    Form,
    Gear,
    Port,
    Cord,
    Plan,
    Play,
    Host,
    Capability,
    Line,
    Manifestation,
    Route,
    Diagnostic,
    Sign,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationPropertyValue {
    Identity(String),
    ConnectionBase(ConnectionBase),
    Text(String),
    Count(u64),
    Flag(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationProperty {
    pub subject: String,
    pub name: String,
    pub value: PresentationPropertyValue,
}

/// One immutable semantic presentation revision.
///
/// Geometry, viewport, toolkit objects, window handles, DOM identity, pixel
/// storage, and base resources are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presentation {
    pub identity: PresentationContentId,
    pub revision: u64,
    pub basis: PresentationBasis,
    pub subjects: Vec<PresentationSubject>,
    pub relationships: Vec<PresentationRelationship>,
    pub properties: Vec<PresentationProperty>,
    pub text: Vec<PresentationText>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationError {
    EmptySubjects,
    TooManySubjects,
    TooManyRelationships,
    TooManyTextItems,
    TooManyProperties,
    TooManySigns,
    EmptyIdentity,
    IdentityTooLong,
    EmptyText,
    TextTooLong,
    DuplicateSubject,
    UnknownRelationshipSubject,
    UnknownTextSubject,
    UnknownPropertySubject,
    DuplicateSign,
    NonCanonicalSign,
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
        properties: Vec<PresentationProperty>,
        text: Vec<PresentationText>,
    ) -> Result<Self, PresentationError> {
        basis.sign_ids.sort();
        if basis.sign_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(PresentationError::DuplicateSign);
        }
        let mut value = Self {
            identity: PresentationContentId(String::new()),
            revision,
            basis,
            subjects,
            relationships,
            properties,
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
        if self.properties.len() > MAX_PRESENTATION_PROPERTIES {
            return Err(PresentationError::TooManyProperties);
        }
        if self.basis.sign_ids.len() > MAX_PRESENTATION_SIGNS {
            return Err(PresentationError::TooManySigns);
        }
        if self
            .basis
            .sign_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(PresentationError::NonCanonicalSign);
        }
        for identity in [
            self.basis.seed_id.as_str(),
            self.basis.body_id.as_str(),
            self.basis.wake_id.as_str(),
            self.basis.source_document_id.as_str(),
            self.basis.checked_form_id.as_str(),
        ] {
            validate_id(identity)?;
        }
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
        if (self.basis.plan_id.is_some() && self.basis.expanded_form_id.is_none())
            || (self.basis.active_play_id.is_some() && self.basis.plan_id.is_none())
        {
            return Err(PresentationError::InvalidBasis);
        }
        for sign in &self.basis.sign_ids {
            validate_id(sign.as_str())?;
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
        for property in &self.properties {
            if !self.has_subject(&property.subject) {
                return Err(PresentationError::UnknownPropertySubject);
            }
            validate_id(&property.name)?;
            match &property.value {
                PresentationPropertyValue::Identity(value) => validate_id(value)?,
                PresentationPropertyValue::Text(value) => validate_text(value)?,
                PresentationPropertyValue::ConnectionBase(_)
                | PresentationPropertyValue::Count(_)
                | PresentationPropertyValue::Flag(_) => {}
            }
        }
        let total_bytes = self
            .basis
            .seed_id
            .as_str()
            .len()
            .saturating_add(self.basis.body_id.as_str().len())
            .saturating_add(self.basis.wake_id.as_str().len())
            .saturating_add(self.basis.source_document_id.as_str().len())
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
                    .sign_ids
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
                self.properties
                    .iter()
                    .map(|property| {
                        property.subject.len()
                            + property.name.len()
                            + property_value_len(&property.value)
                    })
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
        hash_string(&mut digest, self.basis.seed_id.as_str());
        hash_string(&mut digest, self.basis.body_id.as_str());
        hash_string(&mut digest, self.basis.wake_id.as_str());
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
                PresentationPropertyValue::Flag(value) => {
                    digest.update([4, u8::from(*value)]);
                }
            }
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

fn property_value_len(value: &PresentationPropertyValue) -> usize {
    match value {
        PresentationPropertyValue::Identity(value) | PresentationPropertyValue::Text(value) => {
            value.len()
        }
        PresentationPropertyValue::ConnectionBase(_) => 1,
        PresentationPropertyValue::Count(_) => 8,
        PresentationPropertyValue::Flag(_) => 1,
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

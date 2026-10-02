//! Bounded, implementation-neutral content to be made perceptible.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_body::{BodyId, WakeId};
use conduit_core::{
    ActivePlayId, BaseImplementationId, BoundedResourceRef, CheckedPlotId, ExpandedPlotId, KindId,
    PlanId, SignId, SourceDocumentId,
};
use serde::{Deserialize, Serialize};

use crate::{
    PresentationAction, PresentationDisclosure, PresentationTemporalFact, TemporalReference,
};

pub const MAX_PRESENTATION_SUBJECTS: usize = 1_024;
pub const MAX_PRESENTATION_RELATIONSHIPS: usize = 2_048;
pub const MAX_PRESENTATION_TEXT_ITEMS: usize = 2_048;
pub const MAX_PRESENTATION_PROPERTIES: usize = 4_096;
pub const MAX_PRESENTATION_SIGNS: usize = 1_024;
pub const MAX_PRESENTATION_ID_BYTES: usize = 256;
pub const MAX_PRESENTATION_TEXT_BYTES: usize = 1_024;
pub const MAX_PRESENTATION_TOTAL_BYTES: usize = 512 * 1024;
pub const MAX_PRESENTATION_CONTEXT_BASIS: usize = 16;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PresentationContentId(pub(crate) String);

impl PresentationContentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationBasis {
    pub body_id: Option<BodyId>,
    pub wake_id: Option<WakeId>,
    pub source_document_id: Option<SourceDocumentId>,
    pub checked_plot_id: Option<CheckedPlotId>,
    pub expanded_plot_id: Option<ExpandedPlotId>,
    pub plan_id: Option<PlanId>,
    pub active_play_id: Option<ActivePlayId>,
    pub sign_ids: Vec<SignId>,
}

/// Exact renderer-neutral context for one Face projection.
///
/// The identity distinguishes contexts; the basis explains that identity using
/// relationships already present in the Presentation. Neither field grants
/// authority. Masks and Hosts consume this truth rather than supplying hidden
/// audience or disclosure state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationInteractionContext {
    pub identity: String,
    pub basis: Vec<PresentationContextBasis>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationContextBasis {
    pub source: String,
    pub relationship: PresentationRelationshipKind,
    pub target: String,
}

impl PresentationInteractionContext {
    pub(crate) fn general() -> Self {
        Self {
            identity: "presentation/context/general".into(),
            basis: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PresentationRole {
    Document,
    Body,
    Part,
    Candidate,
    Plot,
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
    Info,
    Region,
    Collection,
    Item,
    TextEntry,
    Status,
    Action,
    /// An exact open domain role not owned by this crate's broad structural
    /// classifications, such as `biology/cell/organelle`.
    Semantic(KindId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationSubject {
    pub identity: String,
    pub role: PresentationRole,
    /// The one ordinary bounded human name for this semantic subject.
    ///
    /// Masks may render, speak, emboss, or expose this through a platform
    /// accessibility API. Those are realizations of the same name rather than
    /// a second accessibility-only truth channel.
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PresentationRelationshipKind {
    Contains,
    Connects,
    Describes,
    Realizes,
    Observes,
    /// An exact open relationship meaning, including semantic order such as
    /// `education/lesson/precedes`.
    Semantic(KindId),
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
    BaseImplementationId(BaseImplementationId),
    Text(String),
    Count(u64),
    Signed(i64),
    Flag(bool),
    /// Exact reusable validation truth, carried without becoming a widget.
    ValueContract(conduit_core::CheckedValueContract),
    /// Canonical encoded `&T`: one exact bounded independently addressable
    /// content generation. It contains no path, URL, handle, or authority.
    Content(Vec<u8>),
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
    #[serde(default = "PresentationInteractionContext::general")]
    pub interaction_context: PresentationInteractionContext,
    pub subjects: Vec<PresentationSubject>,
    pub relationships: Vec<PresentationRelationship>,
    #[serde(default)]
    pub composition: Vec<crate::PresentationCompositionRelation>,
    pub properties: Vec<PresentationProperty>,
    pub text: Vec<PresentationText>,
    pub actions: Vec<PresentationAction>,
    pub disclosures: Vec<PresentationDisclosure>,
    #[serde(default)]
    pub temporal_references: Vec<TemporalReference>,
    #[serde(default)]
    pub temporal_facts: Vec<PresentationTemporalFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationError {
    EmptySubjects,
    TooManySubjects,
    TooManyRelationships,
    TooManyTextItems,
    TooManyProperties,
    TooManySigns,
    TooManyActions,
    TooManyInputs,
    TooManyDisclosures,
    EmptyIdentity,
    IdentityTooLong,
    EmptyText,
    TextTooLong,
    DuplicateSubject,
    UnknownRelationshipSubject,
    UnknownTextSubject,
    UnknownPropertySubject,
    DuplicateSign,
    DuplicateAction,
    DuplicateDisclosure,
    UnknownActionTarget,
    DuplicateInput,
    UnknownInputTarget,
    UnknownInputAction,
    InvalidInputContract,
    UnknownDisclosureSubject,
    ReasonTooLong,
    NonCanonicalSign,
    InvalidBasis,
    TooManyBytes,
    InvalidIdentity,
    TooManyTemporalReferences,
    TooManyTemporalFacts,
    DuplicateTemporalReference,
    UnknownTemporalSubject,
    UnknownTemporalReference,
    UnknownTemporalSign,
    InvalidTemporalInstant,
    IncomparableTemporalInstants,
    TemporalIntervalOverflow,
    InvalidTemporalRelation,
    InvalidSemanticIdentity,
    InvalidContent,
    TooManyCompositionRelations,
    UnknownCompositionSubject,
    InvalidCompositionRelation,
    DuplicateCompositionRelation,
    TooMuchContextBasis,
    InvalidContextBasis,
}

impl core::fmt::Display for PresentationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid portable Presentation: {self:?}")
    }
}
impl Presentation {
    pub fn validate(&self) -> Result<(), PresentationError> {
        self.validate_content()?;
        if self.identity.0 != self.content_digest() {
            return Err(PresentationError::InvalidIdentity);
        }
        Ok(())
    }

    pub(crate) fn validate_content(&self) -> Result<(), PresentationError> {
        validate_id(&self.interaction_context.identity)?;
        if self.interaction_context.basis.len() > MAX_PRESENTATION_CONTEXT_BASIS {
            return Err(PresentationError::TooMuchContextBasis);
        }
        for statement in &self.interaction_context.basis {
            validate_id(&statement.source)?;
            validate_id(&statement.target)?;
            if let PresentationRelationshipKind::Semantic(identity) = &statement.relationship {
                validate_semantic_identity(identity)?;
            }
            if !self.relationships.iter().any(|relationship| {
                relationship.source == statement.source
                    && relationship.target == statement.target
                    && relationship.kind == statement.relationship
            }) {
                return Err(PresentationError::InvalidContextBasis);
            }
        }
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
            self.basis.body_id.as_ref().map(|value| value.as_str()),
            self.basis.wake_id.as_ref().map(|value| value.as_str()),
            self.basis
                .source_document_id
                .as_ref()
                .map(|value| value.as_str()),
            self.basis
                .checked_plot_id
                .as_ref()
                .map(|value| value.as_str()),
            self.basis
                .expanded_plot_id
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
        let embodied = self.basis.body_id.is_some();
        if (self.basis.wake_id.is_some() && !embodied)
            || self.basis.source_document_id.is_some() != self.basis.checked_plot_id.is_some()
            || (self.basis.plan_id.is_some() && self.basis.expanded_plot_id.is_none())
            || (self.basis.active_play_id.is_some() && self.basis.plan_id.is_none())
            || (!embodied
                && (self.basis.expanded_plot_id.is_some()
                    || self.basis.plan_id.is_some()
                    || self.basis.active_play_id.is_some()))
        {
            return Err(PresentationError::InvalidBasis);
        }
        for sign in &self.basis.sign_ids {
            validate_id(sign.as_str())?;
        }
        for subject in &self.subjects {
            validate_id(&subject.identity)?;
            if let PresentationRole::Semantic(identity) = &subject.role {
                validate_semantic_identity(identity)?;
            }
            validate_text(&subject.name)?;
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
            if let PresentationRelationshipKind::Semantic(identity) = &relationship.kind {
                validate_semantic_identity(identity)?;
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
                PresentationPropertyValue::Content(encoded) => {
                    BoundedResourceRef::validate_encoded(encoded)
                        .map_err(|_| PresentationError::InvalidContent)?;
                }
                PresentationPropertyValue::ValueContract(contract) => contract
                    .validate_definition()
                    .map_err(|_| PresentationError::InvalidContent)?,
                PresentationPropertyValue::BaseImplementationId(_)
                | PresentationPropertyValue::Count(_)
                | PresentationPropertyValue::Signed(_)
                | PresentationPropertyValue::Flag(_) => {}
            }
        }
        self.validate_semantics()?;
        self.validate_rhetorical_composition()?;
        self.validate_temporal()?;
        if self.content_bytes() > MAX_PRESENTATION_TOTAL_BYTES {
            return Err(PresentationError::TooManyBytes);
        }
        Ok(())
    }

    /// Exact semantic payload size used for finite Presenter admission.
    pub fn content_bytes(&self) -> usize {
        self.basis
            .body_id
            .as_ref()
            .map_or(0, |id| id.as_str().len())
            .saturating_add(
                self.basis
                    .wake_id
                    .as_ref()
                    .map_or(0, |id| id.as_str().len()),
            )
            .saturating_add(
                self.basis
                    .source_document_id
                    .as_ref()
                    .map_or(0, |id| id.as_str().len()),
            )
            .saturating_add(
                self.basis
                    .checked_plot_id
                    .as_ref()
                    .map_or(0, |id| id.as_str().len()),
            )
            .saturating_add(optional_len(
                self.basis.expanded_plot_id.as_ref().map(|id| id.as_str()),
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
            .saturating_add(self.interaction_context.identity.len())
            .saturating_add(
                self.interaction_context
                    .basis
                    .iter()
                    .map(|statement| {
                        statement.source.len()
                            + statement.target.len()
                            + relationship_kind_len(&statement.relationship)
                    })
                    .sum::<usize>(),
            )
            .saturating_add(
                self.subjects
                    .iter()
                    .map(|subject| subject.identity.len() + subject.name.len() + 1)
                    .sum::<usize>(),
            )
            .saturating_add(
                self.relationships
                    .iter()
                    .map(|relationship| relationship.source.len() + relationship.target.len() + 1)
                    .sum::<usize>(),
            )
            .saturating_add(self.rhetorical_composition_len())
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
            )
            .saturating_add(self.semantics_len())
            .saturating_add(self.temporal_len())
    }

    pub(crate) fn has_subject(&self, identity: &str) -> bool {
        self.subjects
            .iter()
            .any(|subject| subject.identity == identity)
    }
}

pub(crate) fn validate_id(value: &str) -> Result<(), PresentationError> {
    if value.is_empty() {
        Err(PresentationError::EmptyIdentity)
    } else if value.len() > MAX_PRESENTATION_ID_BYTES {
        Err(PresentationError::IdentityTooLong)
    } else {
        Ok(())
    }
}

pub(crate) fn validate_text(value: &str) -> Result<(), PresentationError> {
    if value.is_empty() {
        Err(PresentationError::EmptyText)
    } else if value.len() > MAX_PRESENTATION_TEXT_BYTES {
        Err(PresentationError::TextTooLong)
    } else {
        Ok(())
    }
}

fn validate_semantic_identity(value: &KindId) -> Result<(), PresentationError> {
    validate_id(value.as_str())?;
    if !value.as_str().contains('/') {
        return Err(PresentationError::InvalidSemanticIdentity);
    }
    Ok(())
}

fn optional_len(value: Option<&str>) -> usize {
    value.map_or(0, str::len)
}

fn property_value_len(value: &PresentationPropertyValue) -> usize {
    match value {
        PresentationPropertyValue::Identity(value) | PresentationPropertyValue::Text(value) => {
            value.len()
        }
        PresentationPropertyValue::BaseImplementationId(_) => 1,
        PresentationPropertyValue::Count(_) | PresentationPropertyValue::Signed(_) => 8,
        PresentationPropertyValue::Flag(_) => 1,
        PresentationPropertyValue::Content(encoded) => encoded.len(),
        PresentationPropertyValue::ValueContract(contract) => contract.identity_bytes().len(),
    }
}

fn relationship_kind_len(value: &PresentationRelationshipKind) -> usize {
    match value {
        PresentationRelationshipKind::Semantic(identity) => identity.as_str().len(),
        _ => 1,
    }
}

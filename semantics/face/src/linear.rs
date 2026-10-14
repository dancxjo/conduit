//! Bounded nonvisual projection of one canonical portable Face.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{format_relative_time, Face, FaceContentId, FaceError, FacePropertyValue};

/// Maximum number of records in a linear projection, including basis records.
pub const MAX_LINEAR_FACE_LINES: usize = 10_256;
/// Maximum encoded bytes across all records, including one separator per record.
pub const MAX_LINEAR_FACE_BYTES: usize = 2 * 1024 * 1024;

/// One deterministic, implementation-neutral nonvisual rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearFace {
    pub face_id: FaceContentId,
    pub revision: u64,
    pub lines: Vec<String>,
    pub encoded_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearFaceError {
    InvalidFace(FaceError),
    InvalidNavigation(crate::NavigationRefusal),
    InvalidProjection(crate::ProjectionRefusal),
    TooManyLines,
    TooManyBytes,
}

impl core::fmt::Display for LinearFaceError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cannot render linear Face: {self:?}")
    }
}

/// Render every portable identity and content record without spatial state.
///
/// Strings use their escaped debug representation so embedded whitespace and
/// control characters cannot manufacture records or make field boundaries
/// ambiguous to a nonvisual consumer.
pub fn render_linear_face(face: &Face) -> Result<LinearFace, LinearFaceError> {
    face.validate().map_err(LinearFaceError::InvalidFace)?;

    let mut builder = LinearBuilder::new();
    push_linear_basis(&mut builder, face)?;
    for subject in &face.subjects {
        builder.push(linear_subject(subject))?;
    }
    for relationship in &face.relationships {
        builder.push(linear_relationship(relationship))?;
    }
    for relation in &face.composition {
        builder.push(crate::linear_composition(relation))?;
    }
    for property in &face.properties {
        builder.push(linear_property(property))?;
    }
    for text in &face.text {
        builder.push(linear_text(text))?;
    }
    for action in &face.actions {
        builder.push(linear_action(action))?;
    }
    for disclosure in &face.disclosures {
        builder.push(linear_disclosure(disclosure))?;
    }
    for reference in &face.temporal_references {
        builder.push(linear_temporal_reference(reference))?;
    }
    for fact in &face.temporal_facts {
        for line in linear_temporal_fact(fact) {
            builder.push(line)?;
        }
    }

    Ok(builder.finish(face))
}

pub(crate) fn push_linear_basis(
    builder: &mut LinearBuilder,
    face: &Face,
) -> Result<(), LinearFaceError> {
    let basis = &face.basis;
    builder.push(format!(
        "FACE {} revision={}",
        face.identity.as_str(),
        face.revision
    ))?;
    builder.push(format!(
        "BODY {} wake={}",
        optional_identity(basis.body_id.as_ref().map(|identity| identity.as_str())),
        optional_identity(basis.wake_id.as_ref().map(|identity| identity.as_str()))
    ))?;
    builder.push(format!(
        "PLOT source={} checked={} expanded={}",
        optional_identity(
            basis
                .source_document_id
                .as_ref()
                .map(|identity| identity.as_str())
        ),
        optional_identity(
            basis
                .checked_plot_id
                .as_ref()
                .map(|identity| identity.as_str())
        ),
        optional_identity(
            basis
                .expanded_plot_id
                .as_ref()
                .map(|identity| identity.as_str())
        )
    ))?;
    builder.push(format!(
        "PLAN {} PLAY {}",
        optional_identity(basis.plan_id.as_ref().map(|identity| identity.as_str())),
        optional_identity(
            basis
                .active_play_id
                .as_ref()
                .map(|identity| identity.as_str())
        )
    ))?;
    builder.push(format!("SIGNS count={}", basis.sign_ids.len()))?;
    for sign_id in &basis.sign_ids {
        builder.push(format!("SIGN id={}", sign_id.as_str()))?;
    }
    builder.push(format!(
        "INTERACTION_CONTEXT id={:?} basis_count={}",
        face.interaction_context.identity,
        face.interaction_context.basis.len()
    ))?;
    for statement in &face.interaction_context.basis {
        builder.push(format!(
            "CONTEXT_BASIS relationship={:?} source={:?} target={:?}",
            statement.relationship, statement.source, statement.target
        ))?;
    }
    Ok(())
}

pub(crate) fn linear_subject(subject: &crate::FaceSubject) -> String {
    format!(
        "SUBJECT role={:?} id={:?} name={:?}",
        subject.role, subject.identity, subject.name
    )
}

pub(crate) fn linear_relationship(relationship: &crate::FaceRelationship) -> String {
    format!(
        "RELATIONSHIP kind={:?} source={:?} target={:?}",
        relationship.kind, relationship.source, relationship.target
    )
}

pub(crate) fn linear_property(property: &crate::FaceProperty) -> String {
    format!(
        "PROPERTY subject={:?} name={:?} value={}",
        property.subject,
        property.name,
        display_property(&property.value)
    )
}

pub(crate) fn linear_text(text: &crate::FaceText) -> String {
    format!("TEXT subject={:?} value={:?}", text.subject, text.text)
}

pub(crate) fn linear_action(action: &crate::FaceAction) -> String {
    format!(
        "ACTION id={:?} intent={:?} target={:?} name={:?} arguments={:?} disclosure={:?} availability={}",
        action.identity,
        action.intent,
        action.target,
        action.name,
        action.arguments,
        action.disclosure,
        display_availability(&action.availability)
    )
}

pub(crate) fn linear_disclosure(disclosure: &crate::FaceDisclosure) -> String {
    format!(
        "DISCLOSURE subject={:?} level={:?}",
        disclosure.subject, disclosure.level
    )
}

pub(crate) fn linear_temporal_reference(reference: &crate::TemporalReference) -> String {
    format!(
            "TEMPORAL_REFERENCE id={:?} ticks={} scale={:?} clock_basis={:?} resolution={} uncertainty={}",
            reference.identity,
            reference.instant.ticks,
            reference.instant.scale,
            reference.instant.clock_basis,
            reference.instant.resolution_ticks,
            reference.instant.uncertainty_ticks
        )
}

pub(crate) fn linear_temporal_fact(fact: &crate::FaceTemporalFact) -> [String; 2] {
    [
        format!(
            "RELATIVE_TIME subject={:?} role={:?} value={:?}",
            fact.subject,
            fact.role,
            format_relative_time(fact)
        ),
        format!(
            "TEMPORAL_FACT subject={:?} role={:?} sign={} reference={:?} source_ticks={} source_scale={:?} source_clock_basis={:?} source_resolution={} source_uncertainty={} relation={:?}",
            fact.subject,
            fact.role,
            optional_identity(fact.sign_id.as_ref().map(|identity| identity.as_str())),
            fact.reference,
            fact.source.ticks,
            fact.source.scale,
            fact.source.clock_basis,
            fact.source.resolution_ticks,
            fact.source.uncertainty_ticks,
            fact.relation
        ),
    ]
}

pub(crate) struct LinearBuilder {
    lines: Vec<String>,
    encoded_bytes: usize,
}

impl LinearBuilder {
    pub(crate) fn new() -> Self {
        Self {
            lines: Vec::new(),
            encoded_bytes: 0,
        }
    }

    pub(crate) fn push(&mut self, line: String) -> Result<(), LinearFaceError> {
        if self.lines.len() >= MAX_LINEAR_FACE_LINES {
            return Err(LinearFaceError::TooManyLines);
        }
        let next_bytes = self
            .encoded_bytes
            .checked_add(line.len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(LinearFaceError::TooManyBytes)?;
        if next_bytes > MAX_LINEAR_FACE_BYTES {
            return Err(LinearFaceError::TooManyBytes);
        }
        self.lines.push(line);
        self.encoded_bytes = next_bytes;
        Ok(())
    }

    pub(crate) fn finish(self, face: &Face) -> LinearFace {
        LinearFace {
            face_id: face.identity.clone(),
            revision: face.revision,
            lines: self.lines,
            encoded_bytes: self.encoded_bytes,
        }
    }
}

fn optional_identity(identity: Option<&str>) -> &str {
    identity.unwrap_or("none")
}

fn display_property(value: &FacePropertyValue) -> String {
    match value {
        FacePropertyValue::Identity(value) => format!("identity:{value:?}"),
        FacePropertyValue::BaseImplementationId(value) => {
            format!("base={}", display_base(value))
        }
        FacePropertyValue::Text(value) => format!("text:{value:?}"),
        FacePropertyValue::Count(value) => format!("count:{value}"),
        FacePropertyValue::Signed(value) => format!("signed:{value}"),
        FacePropertyValue::Flag(value) => format!("flag:{value}"),
        FacePropertyValue::Content(encoded) => {
            let content = conduit_core::BoundedResourceRef::validate_encoded(encoded)
                .expect("validated Face content remains canonical");
            format!(
                "content:profile={:?},class={:?},bytes={},items={:?}",
                content.content_profile,
                content.access_class,
                content.extent.bytes,
                content.extent.items
            )
        }
        FacePropertyValue::TypedValue { contract, bytes } => {
            crate::display_typed_value(contract, bytes)
        }
        FacePropertyValue::ValueContract(contract) => {
            format!("value-contract:{contract:?}")
        }
    }
}

fn display_availability(value: &crate::FaceActionAvailability) -> String {
    match value {
        crate::FaceActionAvailability::Available => "available".into(),
        crate::FaceActionAvailability::Unavailable {
            reason_code,
            explanation,
        } => format!("unavailable code={reason_code:?} explanation={explanation:?}"),
        crate::FaceActionAvailability::Refused {
            reason_code,
            explanation,
        } => format!("refused code={reason_code:?} explanation={explanation:?}"),
    }
}

fn display_base(base: &conduit_core::BaseImplementationId) -> &str {
    base.as_str()
}

#[cfg(test)]
mod tests;

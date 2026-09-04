//! Bounded nonvisual projection of one canonical portable Presentation.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    format_relative_time, Presentation, PresentationContentId, PresentationError,
    PresentationPropertyValue,
};

/// Maximum number of records in a linear projection, including basis records.
pub const MAX_LINEAR_PRESENTATION_LINES: usize = 10_256;
/// Maximum encoded bytes across all records, including one separator per record.
pub const MAX_LINEAR_PRESENTATION_BYTES: usize = 2 * 1024 * 1024;

/// One deterministic, implementation-neutral nonvisual rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearPresentation {
    pub presentation_id: PresentationContentId,
    pub revision: u64,
    pub lines: Vec<String>,
    pub encoded_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinearPresentationError {
    InvalidPresentation(PresentationError),
    InvalidNavigation(crate::NavigationRefusal),
    InvalidProjection(crate::ProjectionRefusal),
    TooManyLines,
    TooManyBytes,
}

impl core::fmt::Display for LinearPresentationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cannot render linear Presentation: {self:?}")
    }
}

/// Render every portable identity and content record without spatial state.
///
/// Strings use their escaped debug representation so embedded whitespace and
/// control characters cannot manufacture records or make field boundaries
/// ambiguous to a nonvisual consumer.
pub fn render_linear_presentation(
    presentation: &Presentation,
) -> Result<LinearPresentation, LinearPresentationError> {
    presentation
        .validate()
        .map_err(LinearPresentationError::InvalidPresentation)?;

    let mut builder = LinearBuilder::new();
    push_linear_basis(&mut builder, presentation)?;
    for subject in &presentation.subjects {
        builder.push(linear_subject(subject))?;
    }
    for relationship in &presentation.relationships {
        builder.push(linear_relationship(relationship))?;
    }
    for property in &presentation.properties {
        builder.push(linear_property(property))?;
    }
    for text in &presentation.text {
        builder.push(linear_text(text))?;
    }
    for action in &presentation.actions {
        builder.push(linear_action(action))?;
    }
    for input in &presentation.inputs {
        builder.push(crate::linear_input(input))?;
    }
    for disclosure in &presentation.disclosures {
        builder.push(linear_disclosure(disclosure))?;
    }
    for reference in &presentation.temporal_references {
        builder.push(linear_temporal_reference(reference))?;
    }
    for fact in &presentation.temporal_facts {
        for line in linear_temporal_fact(fact) {
            builder.push(line)?;
        }
    }

    Ok(builder.finish(presentation))
}

pub(crate) fn push_linear_basis(
    builder: &mut LinearBuilder,
    presentation: &Presentation,
) -> Result<(), LinearPresentationError> {
    let basis = &presentation.basis;
    builder.push(format!(
        "PRESENTATION {} revision={}",
        presentation.identity.as_str(),
        presentation.revision
    ))?;
    builder.push(format!(
        "BODY {} wake={}",
        optional_identity(basis.body_id.as_ref().map(|identity| identity.as_str())),
        optional_identity(basis.wake_id.as_ref().map(|identity| identity.as_str()))
    ))?;
    builder.push(format!(
        "FORM source={} checked={} expanded={}",
        optional_identity(
            basis
                .source_document_id
                .as_ref()
                .map(|identity| identity.as_str())
        ),
        optional_identity(
            basis
                .checked_form_id
                .as_ref()
                .map(|identity| identity.as_str())
        ),
        optional_identity(
            basis
                .expanded_form_id
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
    Ok(())
}

pub(crate) fn linear_subject(subject: &crate::PresentationSubject) -> String {
    format!(
        "SUBJECT role={:?} id={:?} label={:?} accessibility={:?}",
        subject.role, subject.identity, subject.label, subject.accessibility_name
    )
}

pub(crate) fn linear_relationship(relationship: &crate::PresentationRelationship) -> String {
    format!(
        "RELATIONSHIP kind={:?} source={:?} target={:?}",
        relationship.kind, relationship.source, relationship.target
    )
}

pub(crate) fn linear_property(property: &crate::PresentationProperty) -> String {
    format!(
        "PROPERTY subject={:?} name={:?} value={}",
        property.subject,
        property.name,
        display_property(&property.value)
    )
}

pub(crate) fn linear_text(text: &crate::PresentationText) -> String {
    format!("TEXT subject={:?} value={:?}", text.subject, text.text)
}

pub(crate) fn linear_action(action: &crate::PresentationAction) -> String {
    format!(
        "ACTION id={:?} intent={:?} target={:?} label={:?} disclosure={:?} availability={}",
        action.identity,
        action.intent,
        action.target,
        action.label,
        action.disclosure,
        display_availability(&action.availability)
    )
}

pub(crate) fn linear_disclosure(disclosure: &crate::PresentationDisclosure) -> String {
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

pub(crate) fn linear_temporal_fact(fact: &crate::PresentationTemporalFact) -> [String; 2] {
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

    pub(crate) fn push(&mut self, line: String) -> Result<(), LinearPresentationError> {
        if self.lines.len() >= MAX_LINEAR_PRESENTATION_LINES {
            return Err(LinearPresentationError::TooManyLines);
        }
        let next_bytes = self
            .encoded_bytes
            .checked_add(line.len())
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(LinearPresentationError::TooManyBytes)?;
        if next_bytes > MAX_LINEAR_PRESENTATION_BYTES {
            return Err(LinearPresentationError::TooManyBytes);
        }
        self.lines.push(line);
        self.encoded_bytes = next_bytes;
        Ok(())
    }

    pub(crate) fn finish(self, presentation: &Presentation) -> LinearPresentation {
        LinearPresentation {
            presentation_id: presentation.identity.clone(),
            revision: presentation.revision,
            lines: self.lines,
            encoded_bytes: self.encoded_bytes,
        }
    }
}

fn optional_identity(identity: Option<&str>) -> &str {
    identity.unwrap_or("none")
}

fn display_property(value: &PresentationPropertyValue) -> String {
    match value {
        PresentationPropertyValue::Identity(value) => format!("identity:{value:?}"),
        PresentationPropertyValue::BaseImplementationId(value) => {
            format!("base={}", display_base(value))
        }
        PresentationPropertyValue::Text(value) => format!("text:{value:?}"),
        PresentationPropertyValue::Count(value) => format!("count:{value}"),
        PresentationPropertyValue::Signed(value) => format!("signed:{value}"),
        PresentationPropertyValue::Flag(value) => format!("flag:{value}"),
    }
}

fn display_availability(value: &crate::PresentationActionAvailability) -> String {
    match value {
        crate::PresentationActionAvailability::Available => "available".into(),
        crate::PresentationActionAvailability::Unavailable {
            reason_code,
            explanation,
        } => format!("unavailable code={reason_code:?} explanation={explanation:?}"),
        crate::PresentationActionAvailability::Refused {
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

//! Bounded nonvisual projection of one canonical portable Presentation.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{Presentation, PresentationContentId, PresentationError, PresentationPropertyValue};

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

    let basis = &presentation.basis;
    let mut builder = LinearBuilder::new();
    builder.push(format!(
        "PRESENTATION {} revision={}",
        presentation.identity.as_str(),
        presentation.revision
    ))?;
    builder.push(format!(
        "SEED {} body={} wake={}",
        basis.seed_id.as_str(),
        basis.body_id.as_str(),
        basis.wake_id.as_str()
    ))?;
    builder.push(format!(
        "FORM source={} checked={} expanded={}",
        basis.source_document_id.as_str(),
        basis.checked_form_id.as_str(),
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
    for subject in &presentation.subjects {
        builder.push(format!(
            "SUBJECT role={:?} id={:?} label={:?} accessibility={:?}",
            subject.role, subject.identity, subject.label, subject.accessibility_name
        ))?;
    }
    for relationship in &presentation.relationships {
        builder.push(format!(
            "RELATIONSHIP kind={:?} source={:?} target={:?}",
            relationship.kind, relationship.source, relationship.target
        ))?;
    }
    for property in &presentation.properties {
        builder.push(format!(
            "PROPERTY subject={:?} name={:?} value={}",
            property.subject,
            property.name,
            display_property(&property.value)
        ))?;
    }
    for text in &presentation.text {
        builder.push(format!(
            "TEXT subject={:?} value={:?}",
            text.subject, text.text
        ))?;
    }

    Ok(LinearPresentation {
        presentation_id: presentation.identity.clone(),
        revision: presentation.revision,
        lines: builder.lines,
        encoded_bytes: builder.encoded_bytes,
    })
}

struct LinearBuilder {
    lines: Vec<String>,
    encoded_bytes: usize,
}

impl LinearBuilder {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            encoded_bytes: 0,
        }
    }

    fn push(&mut self, line: String) -> Result<(), LinearPresentationError> {
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
}

fn optional_identity(identity: Option<&str>) -> &str {
    identity.unwrap_or("none")
}

fn display_property(value: &PresentationPropertyValue) -> String {
    match value {
        PresentationPropertyValue::Identity(value) => format!("identity:{value:?}"),
        PresentationPropertyValue::ConnectionBase(value) => {
            format!("base={}", display_base(*value))
        }
        PresentationPropertyValue::Text(value) => format!("text:{value:?}"),
        PresentationPropertyValue::Count(value) => format!("count:{value}"),
        PresentationPropertyValue::Flag(value) => format!("flag:{value}"),
    }
}

fn display_base(base: conduit_core::ConnectionBase) -> &'static str {
    match base {
        conduit_core::ConnectionBase::Local => "local",
        conduit_core::ConnectionBase::InMemory => "in-memory",
        conduit_core::ConnectionBase::FixtureFrame => "fixture frame",
        conduit_core::ConnectionBase::FixtureDatagram => "fixture datagram",
        conduit_core::ConnectionBase::WebSocket => "WebSocket",
        conduit_core::ConnectionBase::UsbCdc => "USB CDC",
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use conduit_body::Body;
    use conduit_core::{
        ActivePlayId, CheckedFormId, ConnectionBase, ExpandedFormId, PlanId, SignId,
        SourceDocumentId,
    };

    use super::*;
    use crate::{
        PresentationBasis, PresentationProperty, PresentationRelationship,
        PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
    };

    fn exact_presentation() -> Presentation {
        let source_document_id = SourceDocumentId::from("source/interface-parity");
        let checked_form_id = CheckedFormId::from("checked/interface-parity");
        let body = Body::born(
            source_document_id.clone(),
            checked_form_id.clone(),
            1,
            SignId::from("sign/born"),
        )
        .unwrap();
        let (body, wake) = body.wake(1, SignId::from("sign/woke")).unwrap();
        Presentation::new(
            7,
            PresentationBasis {
                seed_id: body.seed_id,
                body_id: body.body_id,
                wake_id: wake.wake_id,
                source_document_id,
                checked_form_id,
                expanded_form_id: Some(ExpandedFormId::from("expanded/interface-parity")),
                plan_id: Some(PlanId::from("plan/interface-parity")),
                active_play_id: Some(ActivePlayId::from("play/interface-parity")),
                sign_ids: vec![SignId::from("sign/playing"), SignId::from("sign/presented")],
            },
            vec![
                PresentationSubject {
                    identity: "subject/body".into(),
                    role: PresentationRole::Document,
                    label: "Body view".into(),
                    accessibility_name: "Exact Body and Wake view".into(),
                },
                PresentationSubject {
                    identity: "subject/play".into(),
                    role: PresentationRole::Play,
                    label: "Active Play".into(),
                    accessibility_name: "Active Play status".into(),
                },
            ],
            vec![PresentationRelationship {
                source: "subject/body".into(),
                target: "subject/play".into(),
                kind: PresentationRelationshipKind::Contains,
            }],
            vec![PresentationProperty {
                subject: "subject/play".into(),
                name: "line-base".into(),
                value: PresentationPropertyValue::ConnectionBase(ConnectionBase::UsbCdc),
            }],
            vec![PresentationText {
                subject: "subject/play".into(),
                text: "line one\nline two".into(),
            }],
        )
        .unwrap()
    }

    #[test]
    fn linear_projection_preserves_exact_basis_and_all_nonspatial_content() {
        let presentation = exact_presentation();
        let linear = render_linear_presentation(&presentation).unwrap();
        let output = linear.lines.join("\n");

        assert_eq!(linear.presentation_id, presentation.identity);
        assert_eq!(linear.revision, presentation.revision);
        for identity in [
            presentation.identity.as_str(),
            presentation.basis.seed_id.as_str(),
            presentation.basis.body_id.as_str(),
            presentation.basis.wake_id.as_str(),
            presentation.basis.source_document_id.as_str(),
            presentation.basis.checked_form_id.as_str(),
            presentation
                .basis
                .expanded_form_id
                .as_ref()
                .unwrap()
                .as_str(),
            presentation.basis.plan_id.as_ref().unwrap().as_str(),
            presentation.basis.active_play_id.as_ref().unwrap().as_str(),
            presentation.basis.sign_ids[0].as_str(),
            presentation.basis.sign_ids[1].as_str(),
        ] {
            assert!(output.contains(identity));
        }
        assert!(output.contains(
            "SUBJECT role=Play id=\"subject/play\" label=\"Active Play\" accessibility=\"Active Play status\""
        ));
        assert!(output.contains(
            "RELATIONSHIP kind=Contains source=\"subject/body\" target=\"subject/play\""
        ));
        assert!(output
            .contains("PROPERTY subject=\"subject/play\" name=\"line-base\" value=base=USB CDC"));
        assert!(output.contains("TEXT subject=\"subject/play\" value=\"line one\\nline two\""));
        assert!(linear.lines.len() <= MAX_LINEAR_PRESENTATION_LINES);
        assert!(linear.encoded_bytes <= MAX_LINEAR_PRESENTATION_BYTES);
        assert_eq!(
            linear.encoded_bytes,
            linear.lines.iter().map(|line| line.len() + 1).sum()
        );
    }

    #[test]
    fn invalid_content_identity_fails_closed_before_rendering() {
        let mut presentation = exact_presentation();
        presentation.revision += 1;
        assert_eq!(
            render_linear_presentation(&presentation),
            Err(LinearPresentationError::InvalidPresentation(
                PresentationError::InvalidIdentity
            ))
        );
    }

    #[test]
    fn rendering_is_deterministic_and_contains_no_renderer_local_input() {
        let presentation = exact_presentation();
        let first = render_linear_presentation(&presentation).unwrap();
        let renderer_local_state = ("wayland-window-9", 640_u16, 480_u16);
        assert_eq!(
            first,
            render_linear_presentation(&presentation).unwrap(),
            "renderer-local state {renderer_local_state:?} has no projection input"
        );
    }
}

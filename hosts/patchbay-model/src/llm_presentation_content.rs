//! Bounded accumulation for the LLM semantic Presentation projection.

use conduit_presentation::{
    PresentationDisclosure, PresentationDisclosureLevel, PresentationProperty,
    PresentationPropertyValue, PresentationRelationship, PresentationRelationshipKind,
    PresentationRole, PresentationSubject, PresentationText,
};

pub(super) struct Content {
    pub subjects: Vec<PresentationSubject>,
    pub relationships: Vec<PresentationRelationship>,
    pub properties: Vec<PresentationProperty>,
    pub text: Vec<PresentationText>,
    pub disclosures: Vec<PresentationDisclosure>,
}

impl Content {
    pub(super) fn new() -> Self {
        Self {
            subjects: vec![],
            relationships: vec![],
            properties: vec![],
            text: vec![],
            disclosures: vec![],
        }
    }

    pub(super) fn subject(
        &mut self,
        identity: String,
        role: PresentationRole,
        label: impl Into<String>,
        accessibility_name: impl Into<String>,
    ) {
        self.subjects.push(PresentationSubject {
            identity,
            role,
            label: label.into(),
            accessibility_name: accessibility_name.into(),
        });
    }

    pub(super) fn contains(&mut self, source: &str, target: &str) {
        self.relationships.push(PresentationRelationship {
            source: source.into(),
            target: target.into(),
            kind: PresentationRelationshipKind::Contains,
        });
    }

    pub(super) fn describes(&mut self, source: &str, target: &str) {
        self.relationships.push(PresentationRelationship {
            source: source.into(),
            target: target.into(),
            kind: PresentationRelationshipKind::Describes,
        });
    }

    pub(super) fn text(&mut self, subject: &str, text: impl Into<String>) {
        self.text.push(PresentationText {
            subject: subject.into(),
            text: text.into(),
        });
    }

    fn property(&mut self, subject: &str, name: &str, value: PresentationPropertyValue) {
        self.properties.push(PresentationProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }

    pub(super) fn identity(&mut self, subject: &str, name: &str, value: &str) {
        self.property(
            subject,
            name,
            PresentationPropertyValue::Identity(value.into()),
        );
    }

    pub(super) fn text_property(&mut self, subject: &str, name: &str, value: impl Into<String>) {
        self.property(subject, name, PresentationPropertyValue::Text(value.into()));
    }

    pub(super) fn count(&mut self, subject: &str, name: &str, value: u64) {
        self.property(subject, name, PresentationPropertyValue::Count(value));
    }

    pub(super) fn disclose(&mut self, subject: &str, level: PresentationDisclosureLevel) {
        self.disclosures.push(PresentationDisclosure {
            subject: subject.into(),
            level,
        });
    }
}

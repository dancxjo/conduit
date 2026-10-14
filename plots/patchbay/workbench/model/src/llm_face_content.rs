//! Bounded accumulation for the LLM semantic Face projection.

use conduit_face::{
    FaceDisclosure, FaceDisclosureLevel, FaceProperty, FacePropertyValue, FaceRelationship,
    FaceRelationshipKind, FaceRole, FaceSubject, FaceText,
};

pub(crate) struct Content {
    pub subjects: Vec<FaceSubject>,
    pub relationships: Vec<FaceRelationship>,
    pub properties: Vec<FaceProperty>,
    pub text: Vec<FaceText>,
    pub disclosures: Vec<FaceDisclosure>,
}

impl Content {
    pub(crate) fn new() -> Self {
        Self {
            subjects: vec![],
            relationships: vec![],
            properties: vec![],
            text: vec![],
            disclosures: vec![],
        }
    }

    pub(crate) fn subject(&mut self, identity: String, role: FaceRole, name: impl Into<String>) {
        self.subjects.push(FaceSubject {
            identity,
            role,
            name: name.into(),
        });
    }

    pub(crate) fn contains(&mut self, source: &str, target: &str) {
        self.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind: FaceRelationshipKind::Contains,
        });
    }

    pub(crate) fn connects(&mut self, source: &str, target: &str) {
        self.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind: FaceRelationshipKind::Connects,
        });
    }

    pub(crate) fn describes(&mut self, source: &str, target: &str) {
        self.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind: FaceRelationshipKind::Describes,
        });
    }

    pub(crate) fn text(&mut self, subject: &str, text: impl Into<String>) {
        self.text.push(FaceText {
            subject: subject.into(),
            text: text.into(),
        });
    }

    fn property(&mut self, subject: &str, name: &str, value: FacePropertyValue) {
        self.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }

    pub(crate) fn identity(&mut self, subject: &str, name: &str, value: &str) {
        self.property(subject, name, FacePropertyValue::Identity(value.into()));
    }

    pub(crate) fn text_property(&mut self, subject: &str, name: &str, value: impl Into<String>) {
        self.property(subject, name, FacePropertyValue::Text(value.into()));
    }

    pub(crate) fn count(&mut self, subject: &str, name: &str, value: u64) {
        self.property(subject, name, FacePropertyValue::Count(value));
    }

    pub(crate) fn disclose(&mut self, subject: &str, level: FaceDisclosureLevel) {
        self.disclosures.push(FaceDisclosure {
            subject: subject.into(),
            level,
        });
    }
}

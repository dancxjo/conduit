//! Shared bounded content accumulator for portable Patchbay projections.

use conduit_presentation::{
    PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
};

pub(super) struct ContentBuilder {
    pub(super) subjects: Vec<PresentationSubject>,
    pub(super) relationships: Vec<PresentationRelationship>,
    pub(super) properties: Vec<PresentationProperty>,
    pub(super) text: Vec<PresentationText>,
}

impl ContentBuilder {
    pub(super) fn new() -> Self {
        Self {
            subjects: Vec::new(),
            relationships: Vec::new(),
            properties: Vec::new(),
            text: Vec::new(),
        }
    }

    pub(super) fn from_parts(
        subjects: Vec<PresentationSubject>,
        relationships: Vec<PresentationRelationship>,
        properties: Vec<PresentationProperty>,
        text: Vec<PresentationText>,
    ) -> Self {
        Self {
            subjects,
            relationships,
            properties,
            text,
        }
    }

    pub(super) fn subject(
        &mut self,
        role: PresentationRole,
        label: impl Into<String>,
        accessibility_name: impl Into<String>,
    ) -> String {
        let identity = format!("patchbay/subject/{}", self.subjects.len());
        self.subject_with_identity(identity, role, label, accessibility_name)
    }

    pub(super) fn subject_with_identity(
        &mut self,
        identity: impl Into<String>,
        role: PresentationRole,
        label: impl Into<String>,
        accessibility_name: impl Into<String>,
    ) -> String {
        let identity = identity.into();
        if let Some(existing) = self
            .subjects
            .iter()
            .find(|subject| subject.identity == identity)
        {
            debug_assert_eq!(existing.role, role);
            return identity;
        }
        self.subjects.push(PresentationSubject {
            identity: identity.clone(),
            role,
            label: nonempty(label.into()),
            accessibility_name: nonempty(accessibility_name.into()),
        });
        identity
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

    pub(super) fn line(&mut self, subject: &str, value: impl Into<String>) {
        self.text.push(PresentationText {
            subject: subject.into(),
            text: nonempty(value.into()),
        });
    }

    pub(super) fn property(&mut self, subject: &str, name: &str, value: PresentationPropertyValue) {
        if self.properties.iter().any(|property| {
            property.subject == subject && property.name == name && property.value == value
        }) {
            return;
        }
        self.properties.push(PresentationProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        });
    }
}

fn nonempty(value: String) -> String {
    if value.is_empty() {
        "unavailable".into()
    } else {
        value
    }
}

//! Shared bounded content accumulator for portable Patchbay projections.

use conduit_face::{
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};

pub(super) struct ContentBuilder {
    pub(super) subjects: Vec<FaceSubject>,
    pub(super) relationships: Vec<FaceRelationship>,
    pub(super) properties: Vec<FaceProperty>,
    pub(super) text: Vec<FaceText>,
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
        subjects: Vec<FaceSubject>,
        relationships: Vec<FaceRelationship>,
        properties: Vec<FaceProperty>,
        text: Vec<FaceText>,
    ) -> Self {
        Self {
            subjects,
            relationships,
            properties,
            text,
        }
    }

    pub(super) fn subject(&mut self, role: FaceRole, name: impl Into<String>) -> String {
        let identity = format!("patchbay/subject/{}", self.subjects.len());
        self.subject_with_identity(identity, role, name)
    }

    pub(super) fn subject_with_identity(
        &mut self,
        identity: impl Into<String>,
        role: FaceRole,
        name: impl Into<String>,
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
        self.subjects.push(FaceSubject {
            identity: identity.clone(),
            role,
            name: nonempty(name.into()),
        });
        identity
    }

    pub(super) fn contains(&mut self, source: &str, target: &str) {
        self.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind: FaceRelationshipKind::Contains,
        });
    }

    pub(super) fn describes(&mut self, source: &str, target: &str) {
        self.relationships.push(FaceRelationship {
            source: source.into(),
            target: target.into(),
            kind: FaceRelationshipKind::Describes,
        });
    }

    pub(super) fn line(&mut self, subject: &str, value: impl Into<String>) {
        let value = nonempty(value.into());
        let mut rest = value.as_str();
        while !rest.is_empty() {
            let mut end = rest.len().min(conduit_face::MAX_FACE_TEXT_BYTES);
            while !rest.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push(FaceText {
                subject: subject.into(),
                text: rest[..end].into(),
            });
            rest = &rest[end..];
        }
    }

    pub(super) fn property(&mut self, subject: &str, name: &str, value: FacePropertyValue) {
        if self.properties.iter().any(|property| {
            property.subject == subject && property.name == name && property.value == value
        }) {
            return;
        }
        self.properties.push(FaceProperty {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transcript_continuation_retains_complete_unicode_and_field_limits() {
        let original = format!("{}🪞{}", "x".repeat(1023), "世界Δοκιμή".repeat(200));
        let mut content = ContentBuilder::new();
        content.line("fixture/play", &original);
        assert!(content
            .text
            .iter()
            .all(|line| line.subject == "fixture/play"
                && line.text.len() <= conduit_face::MAX_FACE_TEXT_BYTES));
        assert_eq!(
            content
                .text
                .iter()
                .map(|line| line.text.as_str())
                .collect::<String>(),
            original
        );
    }
}

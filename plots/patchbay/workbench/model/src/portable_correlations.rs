//! Exact planned correlations between logical Program and physical Body subjects.

use conduit_face::{FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole};

use crate::portable_content::ContentBuilder;

pub(super) fn append_planned_correlations(content: &mut ContentBuilder) {
    let subjects = content.subjects.clone();
    for subject in &subjects {
        let target = match subject.role {
            FaceRole::Gear => gear_host(content, &subject.identity),
            FaceRole::Cord => property_identity(content, &subject.identity, "line-id")
                .and_then(|line| subject_with_property(content, FaceRole::Line, "line-id", line)),
            _ => None,
        };
        let Some(target) = target else { continue };
        if !content.relationships.iter().any(|relationship| {
            relationship.source == subject.identity
                && relationship.target == target
                && relationship.kind == FaceRelationshipKind::Realizes
        }) {
            content.relationships.push(FaceRelationship {
                source: subject.identity.clone(),
                target,
                kind: FaceRelationshipKind::Realizes,
            });
        }
    }
}

fn gear_host(content: &ContentBuilder, gear: &str) -> Option<String> {
    let host = property_identity(content, gear, "host-id")?;
    let boot = property_identity(content, gear, "boot-id")?;
    content.subjects.iter().find_map(|subject| {
        (subject.role == FaceRole::Host
            && property_identity(content, &subject.identity, "host-id") == Some(host)
            && property_identity(content, &subject.identity, "boot-id") == Some(boot))
        .then(|| subject.identity.clone())
    })
}

fn subject_with_property(
    content: &ContentBuilder,
    role: FaceRole,
    name: &str,
    value: &str,
) -> Option<String> {
    content.subjects.iter().find_map(|subject| {
        (subject.role == role && property_identity(content, &subject.identity, name) == Some(value))
            .then(|| subject.identity.clone())
    })
}

fn property_identity<'a>(
    content: &'a ContentBuilder,
    subject: &str,
    name: &str,
) -> Option<&'a str> {
    content.properties.iter().find_map(|property| {
        if property.subject != subject || property.name != name {
            return None;
        }
        match &property.value {
            FacePropertyValue::Identity(value) => Some(value.as_str()),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_plan_properties_correlate_only_matching_host_and_line_subjects() {
        let mut content = ContentBuilder::new();
        let gear = content.subject_with_identity("gear/a", FaceRole::Gear, "Gear A");
        let cord = content.subject_with_identity("cord/a", FaceRole::Cord, "Cord A");
        let host = content.subject_with_identity("host/a", FaceRole::Host, "Host A");
        let line = content.subject_with_identity("line/a", FaceRole::Line, "Line A");
        let other = content.subject_with_identity("host/b", FaceRole::Host, "Other Host");
        for (subject, name, value) in [
            (&gear, "host-id", "host/a"),
            (&gear, "boot-id", "boot/a"),
            (&host, "host-id", "host/a"),
            (&host, "boot-id", "boot/a"),
            (&other, "host-id", "host/b"),
            (&other, "boot-id", "boot/b"),
            (&cord, "line-id", "line/a"),
            (&line, "line-id", "line/a"),
        ] {
            content.property(subject, name, FacePropertyValue::Identity(value.into()));
        }

        append_planned_correlations(&mut content);

        assert!(content.relationships.contains(&FaceRelationship {
            source: gear,
            target: host,
            kind: FaceRelationshipKind::Realizes,
        }));
        assert!(content.relationships.contains(&FaceRelationship {
            source: cord,
            target: line,
            kind: FaceRelationshipKind::Realizes,
        }));
        assert!(!content
            .relationships
            .iter()
            .any(|relationship| relationship.target == other));
    }
}

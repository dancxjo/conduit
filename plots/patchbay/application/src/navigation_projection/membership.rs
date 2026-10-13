//! Assign exact face items to workbench Place × Aspect projections.

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use conduit_face::{
    Face, FaceAspect, FaceDepth, FaceDisclosureLevel, FaceNavigation, FacePlace, FaceProperty,
    FaceRole, ProjectionItem, ProjectionMembership,
};

use super::planned_location;

pub(super) fn memberships(
    face: &Face,
    navigation: &FaceNavigation,
) -> Result<Vec<ProjectionMembership>, String> {
    let mut memberships = Vec::new();
    for place in &navigation.places {
        for aspect in &place.aspects {
            for subject in &aspect.focusable_subjects {
                let depth = if place.place == FacePlace::Entrance
                    && face.subjects.iter().any(|candidate| {
                        candidate.identity == *subject && candidate.role == FaceRole::Plot
                    }) {
                    FaceDepth::Primary
                } else {
                    subject_depth(face, subject)
                };
                push(
                    &mut memberships,
                    place.place,
                    aspect.aspect,
                    ProjectionItem::Subject(subject.clone()),
                    depth,
                );
                for (index, property) in face.properties.iter().enumerate() {
                    if property.subject == *subject
                        && property_belongs_to_aspect(
                            property,
                            aspect.aspect,
                            face.subjects
                                .iter()
                                .find(|candidate| candidate.identity == *subject)
                                .map(|candidate| candidate.role.clone()),
                        )
                    {
                        push(
                            &mut memberships,
                            place.place,
                            aspect.aspect,
                            ProjectionItem::Property(ordinal(index)?),
                            property_depth(property),
                        );
                    }
                }
                for (index, text) in face.text.iter().enumerate() {
                    if text.subject == *subject {
                        push(
                            &mut memberships,
                            place.place,
                            aspect.aspect,
                            ProjectionItem::Text(ordinal(index)?),
                            FaceDepth::Context,
                        );
                    }
                }
                for action in &face.actions {
                    if action.target == *subject && aspect.aspect == FaceAspect::Structure {
                        push(
                            &mut memberships,
                            place.place,
                            aspect.aspect,
                            ProjectionItem::Action(action.identity.clone()),
                            disclosure_depth(action.disclosure),
                        );
                    }
                }
            }
            for (index, relationship) in face.relationships.iter().enumerate() {
                if aspect
                    .focusable_subjects
                    .iter()
                    .any(|subject| subject == &relationship.source)
                    && aspect
                        .focusable_subjects
                        .iter()
                        .any(|subject| subject == &relationship.target)
                {
                    push(
                        &mut memberships,
                        place.place,
                        aspect.aspect,
                        ProjectionItem::Relationship(ordinal(index)?),
                        FaceDepth::Primary,
                    );
                }
            }
        }
    }
    for follow in &navigation.follows {
        let index = face
            .relationships
            .iter()
            .position(|relationship| {
                relationship.kind == follow.relationship
                    && ((relationship.source == follow.source_subject
                        && relationship.target == follow.target_subject)
                        || (relationship.source == follow.target_subject
                            && relationship.target == follow.source_subject))
            })
            .ok_or_else(|| "Patchbay FOLLOW relationship is absent".to_owned())?;
        let (place, aspect) = planned_location(&navigation.places, &follow.source_subject)
            .ok_or_else(|| "Patchbay FOLLOW source is not projected".to_owned())?;
        push(
            &mut memberships,
            place,
            aspect,
            ProjectionItem::Relationship(ordinal(index)?),
            FaceDepth::Primary,
        );
    }
    Ok(memberships)
}

fn push(
    memberships: &mut Vec<ProjectionMembership>,
    place: FacePlace,
    aspect: FaceAspect,
    item: ProjectionItem,
    depth: FaceDepth,
) {
    memberships.push(ProjectionMembership {
        place,
        aspect,
        item,
        depth,
    });
}

fn subject_depth(face: &Face, subject: &str) -> FaceDepth {
    face.disclosures
        .iter()
        .find(|disclosure| disclosure.subject == subject)
        .map_or(FaceDepth::Primary, |disclosure| {
            disclosure_depth(disclosure.level)
        })
}

fn disclosure_depth(level: FaceDisclosureLevel) -> FaceDepth {
    match level {
        FaceDisclosureLevel::Primary | FaceDisclosureLevel::CurrentAction => FaceDepth::Primary,
        FaceDisclosureLevel::Context => FaceDepth::Context,
        FaceDisclosureLevel::SelectedDetail => FaceDepth::Detail,
        FaceDisclosureLevel::ExactProvenance => FaceDepth::Exact,
    }
}

fn property_depth(property: &FaceProperty) -> FaceDepth {
    if property.name.ends_with("-id")
        || property.name == "semantic-id"
        || property.name == "source-port"
        || property.name == "sink-port"
    {
        FaceDepth::Exact
    } else {
        FaceDepth::Detail
    }
}

fn property_belongs_to_aspect(
    property: &FaceProperty,
    aspect: FaceAspect,
    role: Option<FaceRole>,
) -> bool {
    if role == Some(FaceRole::Sign) {
        return aspect == FaceAspect::Signs;
    }
    if !matches!(role, Some(FaceRole::Gear | FaceRole::Cord)) {
        return aspect == FaceAspect::Structure;
    }
    let name = property.name.as_str();
    let planned = matches!(
        name,
        "plan-id"
            | "plan-status"
            | "realization-layer"
            | "placement-id"
            | "host-id"
            | "boot-id"
            | "capability-id"
            | "execution-profile-id"
            | "implementation-id"
            | "artifact-id"
            | "runtime-name"
            | "runtime-version"
            | "model-name"
            | "model-content-id"
            | "quantization"
            | "offer-generation"
            | "admitted-capacity"
            | "vector-search-proof-class"
            | "vector-index-resource"
            | "line-id"
            | "line"
            | "base"
            | "base-instance-id"
    ) || name.starts_with("resource-");
    let playing =
        matches!(name, "active-play-id" | "play-state" | "pressure") || name.starts_with("sign-");

    if planned {
        aspect == FaceAspect::Plan
    } else if playing {
        aspect == FaceAspect::Play
    } else {
        aspect == FaceAspect::Structure
    }
}

fn ordinal(index: usize) -> Result<u16, String> {
    u16::try_from(index).map_err(|_| "Face item ordinal exceeds u16".to_owned())
}

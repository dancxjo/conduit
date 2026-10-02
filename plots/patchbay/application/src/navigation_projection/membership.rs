//! Assign exact presentation items to workbench Place × Aspect projections.

use alloc::{borrow::ToOwned, string::String, vec::Vec};
use conduit_presentation::{
    Presentation, PresentationAspect, PresentationDepth, PresentationDisclosureLevel,
    PresentationNavigation, PresentationPlace, PresentationProperty, PresentationRole,
    ProjectionItem, ProjectionMembership,
};

use super::planned_location;

pub(super) fn memberships(
    presentation: &Presentation,
    navigation: &PresentationNavigation,
) -> Result<Vec<ProjectionMembership>, String> {
    let mut memberships = Vec::new();
    for place in &navigation.places {
        for aspect in &place.aspects {
            for subject in &aspect.focusable_subjects {
                let depth = if place.place == PresentationPlace::Entrance
                    && presentation.subjects.iter().any(|candidate| {
                        candidate.identity == *subject && candidate.role == PresentationRole::Plot
                    }) {
                    PresentationDepth::Primary
                } else {
                    subject_depth(presentation, subject)
                };
                push(
                    &mut memberships,
                    place.place,
                    aspect.aspect,
                    ProjectionItem::Subject(subject.clone()),
                    depth,
                );
                for (index, property) in presentation.properties.iter().enumerate() {
                    if property.subject == *subject
                        && property_belongs_to_aspect(
                            property,
                            aspect.aspect,
                            presentation
                                .subjects
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
                for (index, text) in presentation.text.iter().enumerate() {
                    if text.subject == *subject {
                        push(
                            &mut memberships,
                            place.place,
                            aspect.aspect,
                            ProjectionItem::Text(ordinal(index)?),
                            PresentationDepth::Context,
                        );
                    }
                }
                for action in &presentation.actions {
                    if action.target == *subject && aspect.aspect == PresentationAspect::Structure {
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
            for (index, relationship) in presentation.relationships.iter().enumerate() {
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
                        PresentationDepth::Primary,
                    );
                }
            }
        }
    }
    for follow in &navigation.follows {
        let index = presentation
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
            PresentationDepth::Primary,
        );
    }
    Ok(memberships)
}

fn push(
    memberships: &mut Vec<ProjectionMembership>,
    place: PresentationPlace,
    aspect: PresentationAspect,
    item: ProjectionItem,
    depth: PresentationDepth,
) {
    memberships.push(ProjectionMembership {
        place,
        aspect,
        item,
        depth,
    });
}

fn subject_depth(presentation: &Presentation, subject: &str) -> PresentationDepth {
    presentation
        .disclosures
        .iter()
        .find(|disclosure| disclosure.subject == subject)
        .map_or(PresentationDepth::Primary, |disclosure| {
            disclosure_depth(disclosure.level)
        })
}

fn disclosure_depth(level: PresentationDisclosureLevel) -> PresentationDepth {
    match level {
        PresentationDisclosureLevel::Primary | PresentationDisclosureLevel::CurrentAction => {
            PresentationDepth::Primary
        }
        PresentationDisclosureLevel::Context => PresentationDepth::Context,
        PresentationDisclosureLevel::SelectedDetail => PresentationDepth::Detail,
        PresentationDisclosureLevel::ExactProvenance => PresentationDepth::Exact,
    }
}

fn property_depth(property: &PresentationProperty) -> PresentationDepth {
    if property.name.ends_with("-id")
        || property.name == "semantic-id"
        || property.name == "source-port"
        || property.name == "sink-port"
    {
        PresentationDepth::Exact
    } else {
        PresentationDepth::Detail
    }
}

fn property_belongs_to_aspect(
    property: &PresentationProperty,
    aspect: PresentationAspect,
    role: Option<PresentationRole>,
) -> bool {
    if role == Some(PresentationRole::Sign) {
        return aspect == PresentationAspect::Signs;
    }
    if !matches!(role, Some(PresentationRole::Gear | PresentationRole::Cord)) {
        return aspect == PresentationAspect::Structure;
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
        aspect == PresentationAspect::Plan
    } else if playing {
        aspect == PresentationAspect::Play
    } else {
        aspect == PresentationAspect::Structure
    }
}

fn ordinal(index: usize) -> Result<u16, String> {
    u16::try_from(index).map_err(|_| "Presentation item ordinal exceeds u16".to_owned())
}

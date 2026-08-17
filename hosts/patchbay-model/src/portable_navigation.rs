//! Derive portable Place x Aspect navigation from one exact Patchbay Presentation.

use conduit_presentation::{
    NavigationAspect, NavigationPlace, Presentation, PresentationAspect, PresentationCursor,
    PresentationDepth, PresentationDisclosureLevel, PresentationNavigation, PresentationPlace,
    PresentationProjection, PresentationProperty, PresentationRole, ProjectionItem,
    ProjectionMembership,
};

/// One immutable navigation/projection/cursor bundle for a Patchbay revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayNavigationProjection {
    pub navigation: PresentationNavigation,
    pub projection: PresentationProjection,
    pub cursor: PresentationCursor,
}

impl PatchbayNavigationProjection {
    pub fn for_zero_body(presentation: &Presentation, selected_seed: bool) -> Result<Self, String> {
        let entrance_root = presentation
            .properties
            .iter()
            .find(|property| {
                property.name == "this-host"
                    && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
            })
            .map(|property| property.subject.clone())
            .ok_or_else(|| "zero-Body Presentation has no exact entrance Host".to_owned())?;
        let program_root = selected_seed
            .then(|| first_subject(presentation, PresentationRole::Form))
            .flatten();
        let mut places = vec![place(
            presentation,
            PresentationPlace::Entrance,
            entrance_root,
            "Entrance",
        )];
        if let Some(root) = &program_root {
            places.push(place(
                presentation,
                PresentationPlace::Program,
                root.clone(),
                "Program",
            ));
        }
        Self::build(
            presentation,
            places,
            program_root.map_or(PresentationPlace::Entrance, |_| PresentationPlace::Program),
        )
    }

    pub fn for_embodied(presentation: &Presentation) -> Result<Self, String> {
        let program_root = first_subject(presentation, PresentationRole::Form)
            .ok_or_else(|| "embodied Presentation has no Program Form".to_owned())?;
        let body_root = first_subject(presentation, PresentationRole::Body)
            .ok_or_else(|| "embodied Presentation has no Body root".to_owned())?;
        Self::build(
            presentation,
            vec![
                place(
                    presentation,
                    PresentationPlace::Program,
                    program_root,
                    "Program",
                ),
                place(presentation, PresentationPlace::Body, body_root, "Body"),
            ],
            PresentationPlace::Program,
        )
    }

    fn build(
        presentation: &Presentation,
        places: Vec<NavigationPlace>,
        current_place: PresentationPlace,
    ) -> Result<Self, String> {
        let navigation = PresentationNavigation::new(presentation, places, Vec::new())
            .map_err(|error| format!("invalid Patchbay navigation: {error:?}"))?;
        let memberships = memberships(presentation, &navigation)?;
        let projection = PresentationProjection::new(presentation, &navigation, memberships)
            .map_err(|error| format!("invalid Patchbay projection: {error:?}"))?;
        let current = navigation
            .places
            .iter()
            .find(|place| place.place == current_place)
            .ok_or_else(|| "current Patchbay Place is absent".to_owned())?;
        let cursor = PresentationCursor {
            presentation: presentation.identity.clone(),
            navigation: navigation.identity.clone(),
            revision: presentation.revision,
            place: current_place,
            aspect: current.aspects[0].aspect,
            focus: Some(current.root_subject.clone()),
            depth: PresentationDepth::Primary,
        };
        projection
            .project(presentation, &navigation, &cursor)
            .map_err(|error| format!("invalid Patchbay cursor: {error:?}"))?;
        Ok(Self {
            navigation,
            projection,
            cursor,
        })
    }
}

fn place(
    presentation: &Presentation,
    place: PresentationPlace,
    root_subject: String,
    label: &str,
) -> NavigationPlace {
    let aspects = available_aspects(presentation)
        .into_iter()
        .map(|aspect| NavigationAspect {
            aspect,
            focusable_subjects: presentation
                .subjects
                .iter()
                .filter(|subject| subject_in_place(presentation, subject, place, aspect))
                .map(|subject| subject.identity.clone())
                .collect(),
        })
        .collect();
    NavigationPlace {
        place,
        root_subject,
        label: label.into(),
        aspects,
    }
}

fn available_aspects(presentation: &Presentation) -> Vec<PresentationAspect> {
    let mut aspects = vec![PresentationAspect::Structure];
    if presentation.basis.plan_id.is_some() {
        aspects.push(PresentationAspect::Plan);
    }
    if presentation.basis.active_play_id.is_some() {
        aspects.push(PresentationAspect::Play);
    }
    if !presentation.basis.sign_ids.is_empty()
        || presentation
            .subjects
            .iter()
            .any(|subject| subject.role == PresentationRole::Sign)
    {
        aspects.push(PresentationAspect::Signs);
    }
    aspects
}

fn subject_in_place(
    presentation: &Presentation,
    subject: &conduit_presentation::PresentationSubject,
    place: PresentationPlace,
    aspect: PresentationAspect,
) -> bool {
    let role = subject.role;
    if role == PresentationRole::Sign {
        return aspect == PresentationAspect::Signs;
    }
    if role == PresentationRole::Plan {
        return aspect == PresentationAspect::Plan && place != PresentationPlace::Entrance;
    }
    if role == PresentationRole::Play {
        return aspect == PresentationAspect::Play && place != PresentationPlace::Entrance;
    }
    if role == PresentationRole::Diagnostic {
        return place == PresentationPlace::Program && aspect == PresentationAspect::Signs;
    }
    let domain = match role {
        PresentationRole::Seed => Some(PresentationPlace::Entrance),
        PresentationRole::Document
        | PresentationRole::Form
        | PresentationRole::Gear
        | PresentationRole::Port
        | PresentationRole::Cord
        | PresentationRole::Info => Some(PresentationPlace::Program),
        PresentationRole::Body
        | PresentationRole::Part
        | PresentationRole::Candidate
        | PresentationRole::Host
        | PresentationRole::Capability
        | PresentationRole::Line
        | PresentationRole::Route
        | PresentationRole::Manifestation => Some(PresentationPlace::Body),
        PresentationRole::Plan
        | PresentationRole::Play
        | PresentationRole::Diagnostic
        | PresentationRole::Sign => None,
    };
    if place == PresentationPlace::Entrance {
        return aspect == PresentationAspect::Structure
            && matches!(
                role,
                PresentationRole::Seed
                    | PresentationRole::Body
                    | PresentationRole::Host
                    | PresentationRole::Capability
            );
    }
    if place == PresentationPlace::Program && role == PresentationRole::Seed {
        return aspect == PresentationAspect::Structure
            && presentation.properties.iter().any(|property| {
                property.subject == subject.identity
                    && property.name == "opened"
                    && property.value == conduit_presentation::PresentationPropertyValue::Flag(true)
            });
    }
    domain == Some(place) && aspect != PresentationAspect::Signs
}

fn memberships(
    presentation: &Presentation,
    navigation: &PresentationNavigation,
) -> Result<Vec<ProjectionMembership>, String> {
    let mut memberships = Vec::new();
    for place in &navigation.places {
        for aspect in &place.aspects {
            for subject in &aspect.focusable_subjects {
                let depth = if place.place == PresentationPlace::Entrance
                    && presentation.subjects.iter().any(|candidate| {
                        candidate.identity == *subject && candidate.role == PresentationRole::Seed
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
                    if property.subject == *subject {
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

fn first_subject(presentation: &Presentation, role: PresentationRole) -> Option<String> {
    presentation
        .subjects
        .iter()
        .find(|subject| subject.role == role)
        .map(|subject| subject.identity.clone())
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

fn ordinal(index: usize) -> Result<u16, String> {
    u16::try_from(index).map_err(|_| "Presentation item ordinal exceeds u16".to_owned())
}

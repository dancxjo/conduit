//! Derive portable Place x Aspect navigation from one exact Patchbay Face.

use alloc::{borrow::ToOwned, format, string::String, vec, vec::Vec};
use conduit_face::{
    Face, FaceAspect, FaceCursor, FaceDepth, FaceNavigation, FacePlace, FaceProjection,
    FaceRelationshipKind, FaceRole, NavigationAspect, NavigationFollow, NavigationPlace,
};
use serde::{Deserialize, Serialize};

mod membership;
use membership::{memberships, PropertyScope};

/// One immutable navigation/projection/cursor bundle for a Patchbay revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayNavigationProjection {
    pub navigation: FaceNavigation,
    pub projection: FaceProjection,
    pub cursor: FaceCursor,
}

impl PatchbayNavigationProjection {
    /// A structure reader over the exact graph subjects of any Face. Native,
    /// browser and linear readers can share this bundle without sharing geometry.
    /// Root identity and focus order come directly from the retained Face.
    pub fn for_graph(face: &Face) -> Result<Self, String> {
        face.validate()
            .map_err(|error| format!("invalid graph Face: {error:?}"))?;
        let subjects: Vec<_> = face
            .subjects
            .iter()
            .filter(|subject| {
                matches!(
                    subject.role,
                    FaceRole::Gear | FaceRole::Port | FaceRole::Cord
                )
            })
            .map(|subject| subject.identity.clone())
            .collect();
        let root = subjects
            .first()
            .cloned()
            .ok_or_else(|| "Face has no graph subjects".to_owned())?;
        Self::build(
            face,
            vec![NavigationPlace {
                place: FacePlace::Program,
                root_subject: root.clone(),
                name: "Structure".into(),
                aspects: vec![NavigationAspect {
                    aspect: FaceAspect::Structure,
                    focusable_subjects: subjects,
                }],
            }],
            FacePlace::Program,
            root,
            PropertyScope::Complete,
        )
    }

    pub fn for_zero_body(face: &Face, selected_plot: bool) -> Result<Self, String> {
        let entrance_root = face
            .properties
            .iter()
            .find(|property| {
                property.name == "this-host"
                    && property.value == conduit_face::FacePropertyValue::Flag(true)
            })
            .map(|property| property.subject.clone())
            .ok_or_else(|| "zero-body Face has no exact entrance Host".to_owned())?;
        let program_root = selected_plot
            .then(|| {
                face.properties.iter().find_map(|property| {
                    (property.name == "opened"
                        && property.value == conduit_face::FacePropertyValue::Flag(true))
                    .then(|| property.subject.clone())
                })
            })
            .flatten();
        let entrance_focus =
            first_subject(face, FaceRole::Plot).unwrap_or_else(|| entrance_root.clone());
        let mut places = vec![place(face, FacePlace::Entrance, entrance_root, "Entrance")];
        if let Some(root) = &program_root {
            places.push(place(face, FacePlace::Program, root.clone(), "Program"));
        }
        Self::build(
            face,
            places,
            program_root
                .as_ref()
                .map_or(FacePlace::Entrance, |_| FacePlace::Program),
            program_root.unwrap_or(entrance_focus),
            PropertyScope::ByAspect,
        )
    }

    pub fn for_embodied(face: &Face) -> Result<Self, String> {
        let body_root = first_subject(face, FaceRole::Body)
            .ok_or_else(|| "embodied Face has no Body root".to_owned())?;
        let contained_plot = face.relationships.iter().find_map(|relationship| {
            (relationship.source == body_root
                && relationship.kind == FaceRelationshipKind::Contains
                && face.subjects.iter().any(|subject| {
                    subject.identity == relationship.target && subject.role == FaceRole::Plot
                }))
            .then(|| relationship.target.clone())
        });
        let program_root = contained_plot.or_else(|| {
            face.subjects.iter().find_map(|subject| {
                (subject.role == FaceRole::Plot
                    && !face.properties.iter().any(|property| {
                        property.subject == subject.identity
                            && property.name == "workload-membership"
                            && property.value
                                == conduit_face::FacePropertyValue::Text("available".into())
                    }))
                .then(|| subject.identity.clone())
            })
        });
        let Some(program_root) = program_root else {
            return Self::build(
                face,
                vec![place(face, FacePlace::Body, body_root.clone(), "Body")],
                FacePlace::Body,
                body_root,
                PropertyScope::ByAspect,
            );
        };
        Self::build(
            face,
            vec![
                place(face, FacePlace::Body, body_root.clone(), "Body"),
                place(face, FacePlace::Program, program_root.clone(), "Plot"),
            ],
            FacePlace::Body,
            body_root,
            PropertyScope::ByAspect,
        )
    }

    fn build(
        face: &Face,
        places: Vec<NavigationPlace>,
        current_place: FacePlace,
        initial_focus: String,
        property_scope: PropertyScope,
    ) -> Result<Self, String> {
        let follows = follows(face, &places)?;
        let navigation = FaceNavigation::new(face, places, follows)
            .map_err(|error| format!("invalid Patchbay navigation: {error:?}"))?;
        let memberships = memberships(face, &navigation, property_scope)?;
        let projection = FaceProjection::new(face, &navigation, memberships)
            .map_err(|error| format!("invalid Patchbay projection: {error:?}"))?;
        let current = navigation
            .places
            .iter()
            .find(|place| place.place == current_place)
            .ok_or_else(|| "current Patchbay Place is absent".to_owned())?;
        let cursor = FaceCursor {
            face: face.identity.clone(),
            navigation: navigation.identity.clone(),
            revision: face.revision,
            place: current_place,
            aspect: current.aspects[0].aspect,
            focus: Some(initial_focus),
            depth: FaceDepth::Primary,
        };
        projection
            .project(face, &navigation, &cursor)
            .map_err(|error| format!("invalid Patchbay cursor: {error:?}"))?;
        Ok(Self {
            navigation,
            projection,
            cursor,
        })
    }
}

fn follows(face: &Face, places: &[NavigationPlace]) -> Result<Vec<NavigationFollow>, String> {
    let mut follows = Vec::new();
    for (index, relationship) in face.relationships.iter().enumerate() {
        if relationship.kind != FaceRelationshipKind::Realizes {
            continue;
        }
        let Some((source_place, source_aspect)) = planned_location(places, &relationship.source)
        else {
            continue;
        };
        let Some((target_place, target_aspect)) = planned_location(places, &relationship.target)
        else {
            continue;
        };
        if source_place == target_place {
            continue;
        }
        follows.push(NavigationFollow {
            identity: format!("follow/{index}/forward"),
            source_subject: relationship.source.clone(),
            relationship: relationship.kind.clone(),
            target_subject: relationship.target.clone(),
            target_place,
            target_aspect,
        });
        follows.push(NavigationFollow {
            identity: format!("follow/{index}/reverse"),
            source_subject: relationship.target.clone(),
            relationship: relationship.kind.clone(),
            target_subject: relationship.source.clone(),
            target_place: source_place,
            target_aspect: source_aspect,
        });
    }
    if follows.len() > conduit_face::MAX_NAVIGATION_FOLLOWS {
        return Err("Patchbay FOLLOW capacity exceeded".into());
    }
    Ok(follows)
}

fn planned_location(places: &[NavigationPlace], subject: &str) -> Option<(FacePlace, FaceAspect)> {
    places.iter().find_map(|place| {
        place
            .aspects
            .iter()
            .find(|aspect| {
                aspect.aspect == FaceAspect::Plan
                    && aspect
                        .focusable_subjects
                        .iter()
                        .any(|candidate| candidate == subject)
            })
            .map(|aspect| (place.place, aspect.aspect))
    })
}

fn place(face: &Face, place: FacePlace, root_subject: String, label: &str) -> NavigationPlace {
    let aspects = available_aspects(face)
        .into_iter()
        .map(|aspect| NavigationAspect {
            aspect,
            focusable_subjects: face
                .subjects
                .iter()
                .filter(|subject| subject_in_place(face, subject, place, aspect))
                .map(|subject| subject.identity.clone())
                .collect(),
        })
        .collect();
    NavigationPlace {
        place,
        root_subject,
        name: label.into(),
        aspects,
    }
}

fn available_aspects(face: &Face) -> Vec<FaceAspect> {
    let mut aspects = vec![FaceAspect::Structure];
    if face.basis.plan_id.is_some() {
        aspects.push(FaceAspect::Plan);
    }
    if face.basis.active_play_id.is_some() {
        aspects.push(FaceAspect::Play);
    }
    if !face.basis.sign_ids.is_empty()
        || face
            .subjects
            .iter()
            .any(|subject| subject.role == FaceRole::Sign)
    {
        aspects.push(FaceAspect::Signs);
    }
    aspects
}

fn subject_in_place(
    face: &Face,
    subject: &conduit_face::FaceSubject,
    place: FacePlace,
    aspect: FaceAspect,
) -> bool {
    let role = subject.role.clone();
    let has_body = face
        .subjects
        .iter()
        .any(|candidate| candidate.role == FaceRole::Body);
    if role == FaceRole::Sign {
        return aspect == FaceAspect::Signs;
    }
    if role == FaceRole::Plan {
        return aspect == FaceAspect::Plan && place != FacePlace::Entrance;
    }
    if role == FaceRole::Play {
        return aspect == FaceAspect::Play && place != FacePlace::Entrance;
    }
    if role == FaceRole::Diagnostic {
        return place == FacePlace::Program && aspect == FaceAspect::Signs;
    }
    let domain = match role {
        // A plot can be a library entry at the Entrance or the opened Program.
        // Its exact place is established below from the opened property rather
        // than from a privileged face role.
        FaceRole::Plot if has_body => Some(FacePlace::Program),
        FaceRole::Plot => None,
        FaceRole::Document
        | FaceRole::Gear
        | FaceRole::Port
        | FaceRole::Cord
        | FaceRole::Info
        | FaceRole::Region
        | FaceRole::Collection
        | FaceRole::Item
        | FaceRole::TextEntry
        | FaceRole::Status
        | FaceRole::Action
        | FaceRole::Semantic(_) => Some(FacePlace::Program),
        FaceRole::Body
        | FaceRole::Part
        | FaceRole::Candidate
        | FaceRole::Host
        | FaceRole::Capability
        | FaceRole::Line
        | FaceRole::Route
        | FaceRole::Manifestation => Some(FacePlace::Body),
        FaceRole::Plan | FaceRole::Play | FaceRole::Diagnostic | FaceRole::Sign => None,
    };
    if place == FacePlace::Entrance {
        return aspect == FaceAspect::Structure
            && matches!(
                role,
                FaceRole::Plot | FaceRole::Body | FaceRole::Host | FaceRole::Capability
            );
    }
    if place == FacePlace::Program
        && aspect == FaceAspect::Signs
        && matches!(
            role,
            FaceRole::Plot | FaceRole::Gear | FaceRole::Port | FaceRole::Cord
        )
    {
        return true;
    }
    if place == FacePlace::Program && role == FaceRole::Plot {
        return aspect == FaceAspect::Structure
            && (has_body
                || face.properties.iter().any(|property| {
                    property.subject == subject.identity
                        && property.name == "opened"
                        && property.value == conduit_face::FacePropertyValue::Flag(true)
                }));
    }
    if place == FacePlace::Body && role == FaceRole::Plot {
        return aspect == FaceAspect::Structure;
    }
    domain == Some(place) && aspect != FaceAspect::Signs
}

fn first_subject(face: &Face, role: FaceRole) -> Option<String> {
    face.subjects
        .iter()
        .find(|subject| subject.role == role)
        .map(|subject| subject.identity.clone())
}

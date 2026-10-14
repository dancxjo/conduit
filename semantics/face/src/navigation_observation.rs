//! Bounded semantic observation shared by materially different Presenters.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    Face, FaceAction, FaceCursor, FaceNavigation, FaceProjection, FaceSubject, NavigationAspect,
    NavigationFollow, NavigationPlace, ProjectionItem,
};

pub const NAVIGATION_OBSERVATION_SCHEMA: &str = "conduit.face/navigation-observation@1";

/// The exact renderer-neutral facts a Presenter may manifest for one cursor.
///
/// Every collection inherits the finite bounds of validated Face,
/// navigation, and projection truth. Renderer furniture and coordinates are
/// deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationObservation {
    pub schema: String,
    pub face_id: String,
    pub face_revision: u64,
    pub navigation_id: String,
    pub projection_id: String,
    pub cursor: FaceCursor,
    pub available_places: Vec<NavigationPlace>,
    pub available_aspects: Vec<NavigationAspect>,
    pub projected_subjects: Vec<FaceSubject>,
    pub projected_actions: Vec<FaceAction>,
    pub current_follows: Vec<NavigationFollow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationObservationError {
    InvalidFace,
    InvalidNavigation,
    InvalidProjection,
}

pub fn observe_navigation(
    face: &Face,
    navigation: &FaceNavigation,
    projection: &FaceProjection,
    cursor: &FaceCursor,
) -> Result<NavigationObservation, NavigationObservationError> {
    face.validate()
        .map_err(|_| NavigationObservationError::InvalidFace)?;
    navigation
        .validate(face)
        .map_err(|_| NavigationObservationError::InvalidNavigation)?;
    let projected = projection
        .project(face, navigation, cursor)
        .map_err(|_| NavigationObservationError::InvalidProjection)?;
    let current = navigation
        .places
        .iter()
        .find(|place| place.place == cursor.place)
        .ok_or(NavigationObservationError::InvalidNavigation)?;

    let mut projected_subjects = Vec::new();
    let mut projected_actions = Vec::new();
    for membership in projected.items {
        match &membership.item {
            ProjectionItem::Subject(identity) => projected_subjects.push(
                face.subjects
                    .iter()
                    .find(|subject| subject.identity == *identity)
                    .ok_or(NavigationObservationError::InvalidProjection)?
                    .clone(),
            ),
            ProjectionItem::Action(identity) => projected_actions.push(
                face.actions
                    .iter()
                    .find(|action| action.identity == *identity)
                    .ok_or(NavigationObservationError::InvalidProjection)?
                    .clone(),
            ),
            ProjectionItem::Relationship(_)
            | ProjectionItem::Composition(_)
            | ProjectionItem::Property(_)
            | ProjectionItem::Text(_) => {}
        }
    }
    projected_subjects.sort_by(|left, right| left.identity.cmp(&right.identity));
    projected_actions.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut current_follows = cursor
        .focus
        .as_deref()
        .map(|focus| {
            navigation
                .follows
                .iter()
                .filter(|follow| follow.source_subject == focus)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    current_follows.sort_by(|left, right| left.identity.cmp(&right.identity));

    Ok(NavigationObservation {
        schema: NAVIGATION_OBSERVATION_SCHEMA.to_string(),
        face_id: face.identity.as_str().to_string(),
        face_revision: face.revision,
        navigation_id: navigation.identity.as_str().to_string(),
        projection_id: projection.identity.as_str().to_string(),
        cursor: cursor.clone(),
        available_places: navigation.places.clone(),
        available_aspects: current.aspects.clone(),
        projected_subjects,
        projected_actions,
        current_follows,
    })
}

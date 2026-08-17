//! Bounded semantic observation shared by materially different Presenters.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    NavigationAspect, NavigationFollow, NavigationPlace, Presentation, PresentationAction,
    PresentationCursor, PresentationNavigation, PresentationProjection, PresentationSubject,
    ProjectionItem,
};

pub const NAVIGATION_OBSERVATION_SCHEMA: &str = "conduit.presentation/navigation-observation@1";

/// The exact renderer-neutral facts a Presenter may manifest for one cursor.
///
/// Every collection inherits the finite bounds of validated Presentation,
/// navigation, and projection truth. Renderer furniture and coordinates are
/// deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationObservation {
    pub schema: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub navigation_id: String,
    pub projection_id: String,
    pub cursor: PresentationCursor,
    pub available_places: Vec<NavigationPlace>,
    pub available_aspects: Vec<NavigationAspect>,
    pub projected_subjects: Vec<PresentationSubject>,
    pub projected_actions: Vec<PresentationAction>,
    pub current_follows: Vec<NavigationFollow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationObservationError {
    InvalidPresentation,
    InvalidNavigation,
    InvalidProjection,
}

pub fn observe_navigation(
    presentation: &Presentation,
    navigation: &PresentationNavigation,
    projection: &PresentationProjection,
    cursor: &PresentationCursor,
) -> Result<NavigationObservation, NavigationObservationError> {
    presentation
        .validate()
        .map_err(|_| NavigationObservationError::InvalidPresentation)?;
    navigation
        .validate(presentation)
        .map_err(|_| NavigationObservationError::InvalidNavigation)?;
    let projected = projection
        .project(presentation, navigation, cursor)
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
                presentation
                    .subjects
                    .iter()
                    .find(|subject| subject.identity == *identity)
                    .ok_or(NavigationObservationError::InvalidProjection)?
                    .clone(),
            ),
            ProjectionItem::Action(identity) => projected_actions.push(
                presentation
                    .actions
                    .iter()
                    .find(|action| action.identity == *identity)
                    .ok_or(NavigationObservationError::InvalidProjection)?
                    .clone(),
            ),
            ProjectionItem::Relationship(_)
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
        presentation_id: presentation.identity.as_str().to_string(),
        presentation_revision: presentation.revision,
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

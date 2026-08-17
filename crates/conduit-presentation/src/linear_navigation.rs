//! Deterministic nonvisual manifestation of one portable navigation projection.

use alloc::format;

use crate::linear::{
    linear_action, linear_property, linear_relationship, linear_subject, linear_text,
    push_linear_basis, LinearBuilder,
};
use crate::{
    observe_navigation, LinearPresentation, LinearPresentationError, Presentation,
    PresentationCursor, PresentationNavigation, PresentationProjection, ProjectionItem,
};

/// Render exactly the semantic content admitted by one portable cursor.
///
/// The navigation grammar is explicit in the output. Renderer-local keys,
/// geometry, focus rings, and input capability are deliberately absent.
pub fn render_linear_navigation(
    presentation: &Presentation,
    navigation: &PresentationNavigation,
    projection: &PresentationProjection,
    cursor: &PresentationCursor,
) -> Result<LinearPresentation, LinearPresentationError> {
    presentation
        .validate()
        .map_err(LinearPresentationError::InvalidPresentation)?;
    navigation
        .validate(presentation)
        .map_err(LinearPresentationError::InvalidNavigation)?;
    let projected = projection
        .project(presentation, navigation, cursor)
        .map_err(LinearPresentationError::InvalidProjection)?;
    let observation =
        observe_navigation(presentation, navigation, projection, cursor).map_err(|_| {
            LinearPresentationError::InvalidProjection(crate::ProjectionRefusal::InvalidTruth)
        })?;

    let mut builder = LinearBuilder::new();
    push_linear_basis(&mut builder, presentation)?;
    builder.push(format!(
        "NAVIGATION {} projection={} revision={}",
        navigation.identity.as_str(),
        projection.identity.as_str(),
        cursor.revision
    ))?;
    builder.push(format!(
        "CURSOR place={:?} aspect={:?} focus={} depth={:?}",
        cursor.place,
        cursor.aspect,
        cursor.focus.as_deref().unwrap_or("none"),
        cursor.depth
    ))?;
    for place in &observation.available_places {
        builder.push(format!(
            "AVAILABLE PLACE {:?} root={:?} label={:?}",
            place.place, place.root_subject, place.label
        ))?;
    }
    for aspect in &observation.available_aspects {
        builder.push(format!(
            "AVAILABLE ASPECT {:?} focusable={}",
            aspect.aspect,
            aspect.focusable_subjects.len()
        ))?;
    }
    for follow in &observation.current_follows {
        builder.push(format!(
            "AVAILABLE FOLLOW id={:?} relationship={:?} source={:?} target={:?} place={:?} aspect={:?}",
            follow.identity,
            follow.relationship,
            follow.source_subject,
            follow.target_subject,
            follow.target_place,
            follow.target_aspect
        ))?;
    }
    for membership in projected.items {
        let line = match &membership.item {
            ProjectionItem::Subject(identity) => presentation
                .subjects
                .iter()
                .find(|subject| subject.identity == *identity)
                .map(linear_subject),
            ProjectionItem::Relationship(index) => presentation
                .relationships
                .get(usize::from(*index))
                .map(linear_relationship),
            ProjectionItem::Property(index) => presentation
                .properties
                .get(usize::from(*index))
                .map(linear_property),
            ProjectionItem::Text(index) => {
                presentation.text.get(usize::from(*index)).map(linear_text)
            }
            ProjectionItem::Action(identity) => presentation
                .actions
                .iter()
                .find(|action| action.identity == *identity)
                .map(linear_action),
        }
        .expect("validated projection memberships always reference current content");
        builder.push(line)?;
    }
    Ok(builder.finish(presentation))
}

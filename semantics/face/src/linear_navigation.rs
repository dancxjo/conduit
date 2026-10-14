//! Deterministic nonvisual manifestation of one portable navigation projection.

use alloc::format;

use crate::linear::{
    linear_action, linear_property, linear_relationship, linear_subject, linear_text,
    push_linear_basis, LinearBuilder,
};
use crate::{
    observe_navigation, Face, FaceCursor, FaceNavigation, FaceProjection, LinearFace,
    LinearFaceError, ProjectionItem,
};

/// Render exactly the semantic content admitted by one portable cursor.
///
/// The navigation grammar is explicit in the output. Renderer-local keys,
/// geometry, focus rings, and input capability are deliberately absent.
pub fn render_linear_navigation(
    face: &Face,
    navigation: &FaceNavigation,
    projection: &FaceProjection,
    cursor: &FaceCursor,
) -> Result<LinearFace, LinearFaceError> {
    face.validate().map_err(LinearFaceError::InvalidFace)?;
    navigation
        .validate(face)
        .map_err(LinearFaceError::InvalidNavigation)?;
    let projected = projection
        .project(face, navigation, cursor)
        .map_err(LinearFaceError::InvalidProjection)?;
    let observation = observe_navigation(face, navigation, projection, cursor)
        .map_err(|_| LinearFaceError::InvalidProjection(crate::ProjectionRefusal::InvalidTruth))?;

    let mut builder = LinearBuilder::new();
    push_linear_basis(&mut builder, face)?;
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
            place.place, place.root_subject, place.name
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
            ProjectionItem::Subject(identity) => face
                .subjects
                .iter()
                .find(|subject| subject.identity == *identity)
                .map(linear_subject),
            ProjectionItem::Relationship(index) => face
                .relationships
                .get(usize::from(*index))
                .map(linear_relationship),
            ProjectionItem::Composition(index) => face
                .composition
                .get(usize::from(*index))
                .map(crate::linear_composition),
            ProjectionItem::Property(index) => face
                .properties
                .get(usize::from(*index))
                .map(linear_property),
            ProjectionItem::Text(index) => face.text.get(usize::from(*index)).map(linear_text),
            ProjectionItem::Action(identity) => face
                .actions
                .iter()
                .find(|action| action.identity == *identity)
                .map(linear_action),
        }
        .expect("validated projection memberships always reference current content");
        builder.push(line)?;
    }
    Ok(builder.finish(face))
}

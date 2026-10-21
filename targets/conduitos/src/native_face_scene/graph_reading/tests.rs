//! Deterministic semantic reading checks; no browser or guest input enactment.
use super::*;
use conduit_face::ProjectionItem;

fn specimen() -> Face {
    serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap()
}

#[test]
fn exact_subject_reading_retains_endpoint_facts_after_repeated_selection() {
    let face = specimen();
    let subjects: Vec<_> = face
        .subjects
        .iter()
        .filter(|subject| {
            matches!(
                subject.role,
                FaceRole::Gear | FaceRole::Port | FaceRole::Cord
            )
        })
        .collect();
    assert!(!subjects.is_empty());
    let mut scene = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    // More than 64 operations must not exhaust a hidden navigation history.
    for index in 0..(subjects.len() + 80) {
        let subject = subjects[index % subjects.len()];
        scene
            .inspect_subject(&face.identity, face.revision, &subject.identity)
            .unwrap();
        let bundle = scene.graph_projection().unwrap();
        assert_eq!(
            bundle.cursor.focus.as_deref(),
            Some(subject.identity.as_str())
        );
        assert_eq!(bundle.cursor.depth, FaceDepth::Exact);
        assert_eq!(scene.page(), scene.graph_focus_page().unwrap());
        let projected = bundle
            .projection
            .project(&face, &bundle.navigation, &bundle.cursor)
            .unwrap();
        for (ordinal, property) in face
            .properties
            .iter()
            .enumerate()
            .filter(|(_, property)| property.subject == subject.identity)
        {
            assert!(
                projected.items.iter().any(|membership| {
                    membership.item == ProjectionItem::Property(ordinal as u16)
                }),
                "missing exact property {}",
                property.name
            );
        }
        scene.frame().unwrap();
    }
    assert_eq!(scene.face(), &face);
}

#[test]
fn stale_or_unknown_subject_refuses_without_moving_current_reading() {
    let face = specimen();
    let identity = &face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Gear)
        .unwrap()
        .identity;
    let mut scene = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    scene
        .inspect_subject(&face.identity, face.revision, identity)
        .unwrap();
    let cursor = scene.graph_projection().unwrap().cursor.clone();
    let page = scene.page();
    assert_eq!(
        scene.inspect_subject(&face.identity, face.revision + 1, identity),
        Err(FaceSceneError::StaleFace)
    );
    assert_eq!(
        scene.inspect_subject(&face.identity, face.revision, "absent/subject"),
        Err(FaceSceneError::UnknownControl)
    );
    assert_eq!(scene.graph_projection().unwrap().cursor, cursor);
    assert_eq!(scene.page(), page);
    assert_eq!(scene.face(), &face);
}

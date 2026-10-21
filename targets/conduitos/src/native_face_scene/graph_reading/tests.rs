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

#[test]
fn keyboard_reads_exact_subjects_and_returns_to_originating_diagram_page() {
    let face = specimen();
    let show = super::super::tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    scene.show_diagram(true);
    for _ in 1..scene.pages() {
        scene.turn_page(true);
    }
    let origin = scene.page();
    let subjects = scene.graph_projection().unwrap().navigation.places[0].aspects[0]
        .focusable_subjects
        .clone();
    for (index, identity) in subjects.iter().enumerate() {
        let tab = KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
        assert!(matches!(
            scene.key(tab, &show, index as u64 + 1).unwrap(),
            FaceSceneInput::Changed
        ));
        assert_eq!(
            scene.graph_projection().unwrap().cursor.focus.as_deref(),
            Some(identity.as_str())
        );
    }
    let enter = KeyEvent::new(40, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    assert!(matches!(
        scene.key(enter, &show, 1000).unwrap(),
        FaceSceneInput::Unchanged
    ));
    let back = KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::LEFT_SHIFT).unwrap();
    scene.key(back, &show, 1001).unwrap();
    assert_eq!(
        scene.graph_projection().unwrap().cursor.focus.as_deref(),
        Some(subjects[subjects.len() - 2].as_str())
    );
    let escape = KeyEvent::new(41, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    scene.key(escape, &show, 1002).unwrap();
    assert!(scene.showing_diagram);
    assert_eq!(scene.page(), origin);
    assert_eq!(scene.face(), &face);
}

#[test]
fn pointer_uses_emitted_gear_port_and_uncovered_cord_targets() {
    let face = specimen();
    let show = super::super::tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face.clone(), 1280, 800).unwrap();
    scene.show_diagram(true);
    let mut drawing = GraphicsScene::empty();
    let hits = diagram::append(&mut drawing, &face, scene.screen, scene.page()).unwrap();
    let mut targets = Vec::new();
    for (bounds, identity) in hits.gears.iter().chain(hits.ports.iter()).flatten() {
        let point = (bounds.x + 1, bounds.y + 1);
        assert_eq!(hits.hit_test(point.0, point.1), Some(*identity));
        targets.push((String::from(*identity), point));
    }
    for (path, identity) in hits.cords.iter().flatten() {
        let point = path.points().windows(2).find_map(|pair| {
            let (ax, ay) = (*pair[0].x(), *pair[0].y());
            let (bx, by) = (*pair[1].x(), *pair[1].y());
            (ay.min(by)..=ay.max(by)).find_map(|y| {
                (ax.min(bx)..=ax.max(bx))
                    .find_map(|x| (hits.hit_test(x, y) == Some(*identity)).then_some((x, y)))
            })
        });
        if let Some(point) = point {
            targets.push((String::from(*identity), point));
        }
    }
    for role in [FaceRole::Gear, FaceRole::Port, FaceRole::Cord] {
        assert!(targets.iter().any(|(identity, _)| {
            face.subjects
                .iter()
                .any(|subject| subject.identity == *identity && subject.role == role)
        }));
    }
    for (index, (identity, (x, y))) in targets.iter().enumerate() {
        scene.show_diagram(true);
        assert!(matches!(
            scene.pointer(*x, *y, &show, index as u64 + 1).unwrap(),
            FaceSceneInput::Changed
        ));
        assert_eq!(
            scene.graph_projection().unwrap().cursor.focus.as_deref(),
            Some(identity.as_str())
        );
    }
    let cursor = scene.graph_projection().unwrap().cursor.clone();
    let stale = show
        .transition(
            conduit_face::ManifestationLifecycle::Replaced,
            conduit_core::SignId::from("fixture/graph-show-replaced"),
        )
        .unwrap();
    assert!(matches!(
        scene.pointer(0, 0, &stale, 1000),
        Err(FaceSceneError::StaleFace)
    ));
    assert_eq!(scene.graph_projection().unwrap().cursor, cursor);
    assert_eq!(scene.face(), &face);
}

fn scaled_scene(face: &Face, scale: u16) -> NativeFaceScene {
    let mut expression = conduit_face::VisualExpression::phosphor();
    expression.text_scale_percent = scale;
    let admitted = conduit_face::admit_visual_expression(
        Ok(expression),
        conduit_face::VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    NativeFaceScene::prepare_visual_fallback(face.clone(), 1280, 800, true, &admitted).unwrap()
}

#[test]
fn requested_graph_and_exact_focus_restore_after_enlarged_text_fallback() {
    let face = specimen();
    let identity = &face
        .subjects
        .iter()
        .find(|subject| subject.role == FaceRole::Cord)
        .unwrap()
        .identity;
    let mut original = scaled_scene(&face, 100);
    original
        .inspect_subject(&face.identity, face.revision, identity)
        .unwrap();
    original.show_diagram(true);
    for _ in 1..original.pages() {
        original.turn_page(true);
    }
    let page = original.page();
    let mut enlarged = scaled_scene(&face, 200);
    enlarged.retain_encounter(original).unwrap();
    assert!(enlarged.showing_details());
    assert!(!enlarged.showing_diagram());
    assert_eq!(
        enlarged.diagram_refusal(),
        Some(FaceSceneError::InvalidExtent)
    );
    assert_eq!(enlarged.page(), enlarged.graph_focus_page().unwrap());
    assert_eq!(
        enlarged.graph_projection().unwrap().cursor.focus.as_deref(),
        Some(identity.as_str())
    );
    let mut restored = scaled_scene(&face, 100);
    restored.retain_encounter(enlarged).unwrap();
    assert!(restored.showing_diagram());
    assert_eq!(restored.page(), page);
    assert_eq!(restored.diagram_refusal(), None);
    assert_eq!(restored.local_notice, None);
    assert_eq!(
        restored.graph_projection().unwrap().cursor.focus.as_deref(),
        Some(identity.as_str())
    );
    restored.frame().unwrap();
    assert_eq!(restored.face(), &face);
}

#[test]
fn explicitly_leaving_graph_reading_cancels_restore_request() {
    let face = specimen();
    let mut enlarged = scaled_scene(&face, 200);
    enlarged.show_diagram(true);
    assert_eq!(
        enlarged.diagram_refusal(),
        Some(FaceSceneError::InvalidExtent)
    );
    enlarged.show_details(true);
    let mut restored = scaled_scene(&face, 100);
    restored.retain_encounter(enlarged).unwrap();
    assert!(restored.showing_details());
    assert!(!restored.showing_diagram());
    assert_eq!(restored.local_notice, None);
    restored.frame().unwrap();
    assert_eq!(restored.face(), &face);
}

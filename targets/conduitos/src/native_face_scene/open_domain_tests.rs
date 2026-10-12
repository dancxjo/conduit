//! Production native layout over the source-verified domain Face fixture.
//! No scanout, installed-owner acknowledgement, or human observation is claimed.
use super::*;
use alloc::string::String;
use conduit_presentation::MAX_GRAPHICS_COMMANDS;

#[test]
fn native_generic_mask_preserves_open_domain_meaning_and_typed_controls() {
    let face: Presentation = serde_json::from_str(include_str!(
        "../../../../proof/conformance/presentation-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap();
    face.validate().unwrap();
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert_eq!(scene.presentation(), &face);
    assert!(
        !scene.has_diagram(),
        "open roles must not invent a Gear/Port/Cord graph"
    );
    for name in ["The cell", "Mitochondrion", "Answer"] {
        assert!(
            scene.primary.iter().any(|row| row.text.contains(name)),
            "lost {name}"
        );
    }
    let mut primary = String::new();
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        for command in frame.scene.commands() {
            assert_eq!(
                command.clip_class(),
                conduit_presentation::GraphicsClipClass::FullyVisible
            );
            primary.push_str(command.payload());
        }
        scene.turn_page(true);
    }
    assert!(primary.contains("Mitochondrion"));
    scene.show_details(true);
    let details: String = scene.details.iter().map(|row| row.text.as_str()).collect();
    assert!(details.contains("biology/cell/organelle"));
    assert!(details.contains("biology/cell/contains-organelle"));
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .all(|command| command.clip_class()
                    == conduit_presentation::GraphicsClipClass::FullyVisible)
        );
        scene.turn_page(true);
    }
    let action = face
        .actions
        .iter()
        .position(|action| action.identity == "lesson/answer")
        .unwrap();
    let control = FaceControl {
        action,
        argument: Some(0),
    };
    let (resolved, argument) = scene
        .resolve(&face.identity, face.revision, control)
        .unwrap();
    assert_eq!(resolved.identity, "lesson/answer");
    assert_eq!(argument.unwrap().name, "lesson/answer-text");
    assert_eq!(
        scene.resolve(&face.identity, face.revision + 1, control),
        Err(FaceSceneError::StaleFace)
    );
    assert_eq!(
        scene.resolve(
            &face.identity,
            face.revision,
            FaceControl {
                action: usize::MAX,
                argument: None
            }
        ),
        Err(FaceSceneError::UnknownControl)
    );
    assert_eq!(scene.presentation(), &face);
}

#[test]
fn native_generic_mask_preserves_finite_places_without_inventing_connectivity() {
    let face: Presentation = serde_json::from_str(include_str!(
        "../../../../proof/conformance/presentation-waist/domain-derived/finite-place-face.json"
    ))
    .unwrap();
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert!(!scene.has_diagram());
    let mut visible = String::new();
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
        for command in frame.scene.commands() {
            assert_eq!(
                command.clip_class(),
                conduit_presentation::GraphicsClipClass::FullyVisible
            );
            visible.push_str(command.payload());
        }
        scene.turn_page(true);
    }
    for name in ["Hub", "Workshop", "Crèche"] {
        assert!(visible.contains(name), "lost {name}");
    }
    scene.show_details(true);
    let details: String = scene.details.iter().map(|row| row.text.as_str()).collect();
    assert!(details.contains("commons/contains-place"));
    for (index, action) in face
        .actions
        .iter()
        .enumerate()
        .filter(|(_, action)| action.identity.starts_with("commons/"))
    {
        let resolved = scene.resolve(
            &face.identity,
            face.revision,
            FaceControl {
                action: index,
                argument: None,
            },
        );
        if action.availability.is_available() {
            let (current, argument) = resolved.unwrap();
            assert_eq!(current, action);
            assert!(argument.is_none());
        } else {
            assert_eq!(resolved, Err(FaceSceneError::UnavailableAction));
        }
    }
    assert_eq!(scene.presentation(), &face);
}

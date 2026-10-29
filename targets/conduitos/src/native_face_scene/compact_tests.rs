//! Native 240-square geometry uses the ordinary measured scene and CPU renderer.
use super::*;
use alloc::collections::BTreeSet;
use conduit_face::{GraphicsClipClass, GraphicsCommandKind, MAX_GRAPHICS_COMMANDS};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};

fn assert_frame(scene: &NativeFaceScene) {
    let frame = scene.frame().unwrap();
    let chrome = chrome::Chrome::prepare(scene.screen, scene.text_scale_percent).unwrap();
    assert_eq!(frame.face_id, &scene.face.identity);
    assert_eq!(frame.revision, scene.face.revision);
    assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
    for command in frame.scene.commands() {
        assert_eq!(command.clip_class(), GraphicsClipClass::FullyVisible);
        assert!(command.bounds.x >= 0 && command.bounds.y >= 0);
        assert!(u32::from(command.bounds.width) + command.bounds.x as u32 <= 240);
        assert!(u32::from(command.bounds.height) + command.bounds.y as u32 <= 240);
        if command.kind == GraphicsCommandKind::Text {
            let height = crate::display::typography::TextLayout::new_scaled(
                command.payload(),
                command.text_role().into(),
                command.bounds.width,
                scene.text_scale_percent,
            )
            .unwrap()
            .finish_height();
            assert!(
                height <= u32::from(command.bounds.height),
                "clipped text: {}",
                command.payload()
            );
        }
    }
    for hit in &frame.hits {
        assert!(hit.bounds.y >= chrome.top as i16);
        assert!(hit.bounds.y as u16 + hit.bounds.height <= chrome.bottom);
        assert_eq!(
            frame
                .hit_test(hit.bounds.x + 1, hit.bounds.y + 1)
                .unwrap()
                .control,
            hit.control
        );
    }
    let mut pixels = raster_layout_tests::Pixels::new(240, 240);
    let receipt = crate::display::typography::render_scene_scaled(
        &mut pixels,
        &frame.scene,
        |_, command| command.text_role().into(),
        scene.text_scale_percent,
    )
    .unwrap();
    assert_eq!(receipt.commands as usize, frame.scene.commands().len());
    assert!(receipt.pixels_written > 0);
}

#[test]
fn compact_pages_preserve_content_and_controls_without_obscuring_chrome() {
    let face = tests::face(21);
    let mut scene = NativeFaceScene::prepare(face.clone(), 240, 240).unwrap();
    assert_eq!(scene.bounds().width, 240);
    assert!(scene.pages() > 1);
    let expected: BTreeSet<_> = scene
        .primary
        .iter()
        .filter_map(|row| {
            row.control
                .map(|control| (control.action, control.argument))
        })
        .collect();
    let mut seen = BTreeSet::new();
    for page in 0..scene.pages() {
        assert_eq!(scene.page(), page);
        assert_frame(&scene);
        seen.extend(
            scene
                .frame()
                .unwrap()
                .hits
                .iter()
                .map(|hit| (hit.control.action, hit.control.argument)),
        );
        scene.turn_page(true);
    }
    assert_eq!(seen, expected);
    assert_eq!(scene.page(), scene.pages() - 1);
    scene.turn_page(false);
    assert_eq!(scene.page(), scene.pages().saturating_sub(2));
    scene.show_details(true);
    for _ in 0..scene.pages() {
        assert_frame(&scene);
        scene.turn_page(true);
    }
    assert_eq!(scene.face(), &face);
}

#[test]
fn compact_focus_crosses_pages_and_activation_keeps_exact_show_identity() {
    let face = tests::face(22);
    let show = tests::fixture::show(&face);
    let mut scene = NativeFaceScene::prepare(face.clone(), 240, 240).unwrap();
    let controls: BTreeSet<_> = scene
        .primary
        .iter()
        .filter_map(|row| {
            row.control
                .map(|control| (control.action, control.argument))
        })
        .collect();
    let mut visited = BTreeSet::new();
    for _ in 0..controls.len() {
        let control = scene.focus_next(true).unwrap();
        visited.insert((control.action, control.argument));
        assert_frame(&scene);
        assert!(
            scene
                .frame()
                .unwrap()
                .hits
                .iter()
                .any(|hit| hit.control == control)
        );
    }
    assert_eq!(visited, controls);
    for _ in 0..controls.len() {
        if scene
            .focused()
            .is_some_and(|control| face.actions[control.action].identity == "finish")
        {
            break;
        }
        scene.focus_next(true);
    }
    let event = KeyEvent::new(40, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    let FaceSceneInput::Submitted { interaction, .. } = scene.key(event, &show, 7).unwrap() else {
        panic!("focused argument-free action must submit");
    };
    assert_eq!(interaction.action_id, "finish");
    assert_eq!(interaction.face_revision, face.revision);
    assert!(interaction.arguments.is_empty());
    interaction.validate_against(&face, &show).unwrap();
    assert_eq!(scene.face(), &face);
}

#[test]
fn compact_extent_and_unmeasurable_notice_refuse_before_replacing_frame() {
    let face = tests::face(23);
    for (width, height) in [(239, 240), (240, 239)] {
        assert!(matches!(
            NativeFaceScene::prepare(face.clone(), width, height),
            Err(FaceSceneError::InvalidExtent)
        ));
    }
    let mut scene = NativeFaceScene::prepare(face, 240, 240).unwrap();
    let before = scene.frame().unwrap().scene;
    assert_eq!(
        scene.set_local_notice(&"W".repeat(96)),
        Err(FaceSceneError::InputBound)
    );
    assert_eq!(scene.frame().unwrap().scene, before);
}

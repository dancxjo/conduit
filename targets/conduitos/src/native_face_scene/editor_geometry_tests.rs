//! Geometry conformance for the accepted editor facet.
use super::*;

#[test]
fn active_editor_uses_measured_regions_and_retains_exact_long_names_in_details() {
    let original = super::super::tests::face(1);
    let mut subjects = original.subjects;
    let name = format!(
        "Crèche {} {}",
        "e\u{301}界\u{10ffff}".repeat(20),
        "x".repeat(300)
    );
    subjects
        .iter_mut()
        .find(|subject| subject.identity == "z-first")
        .unwrap()
        .name = name.clone();
    let mut properties = original.properties;
    properties.push(FaceProperty {
        subject: "z-first".into(),
        name: "text-editing-active".into(),
        value: FacePropertyValue::Flag(true),
    });
    let mut text = original.text;
    text.push(FaceText {
        subject: "z-first".into(),
        text: "é\u{301}界".repeat(100),
    });
    let face = Face::new_with_semantics(
        original.revision,
        original.basis,
        subjects,
        original.relationships,
        properties,
        text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    for (width, height) in [(1280, 800), (640, 480), (320, 240)] {
        for scale in [100, 200] {
            let mut scene = NativeFaceScene::prepare_visual_fallback(
                face.clone(),
                width,
                height,
                true,
                &expression(scale, VisualDensity::Spacious),
            )
            .unwrap();
            let frame = scene.frame().unwrap();
            let chrome = chrome::Chrome::prepare(scene.screen, scale).unwrap();
            assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
            assert!(
                frame.hits.is_empty(),
                "editor decoration invents no action hit"
            );
            let caption = frame
                .scene
                .commands()
                .iter()
                .find(|command| {
                    command.kind == GraphicsCommandKind::Text
                        && command.payload().ends_with("(end)")
                })
                .unwrap();
            assert!(caption.payload().contains('…'));
            assert!(caption.payload().len() <= MAX_GRAPHICS_TEXT_BYTES);
            assert!(caption.bounds.y >= chrome.top as i16);
            assert!(
                TextLayout::new_scaled(
                    caption.payload(),
                    caption.text_role().into(),
                    caption.bounds.width,
                    scale
                )
                .unwrap()
                .finish_height()
                    <= u32::from(caption.bounds.height)
            );
            assert!(
                frame
                    .scene
                    .commands()
                    .iter()
                    .filter(|command| command.kind == GraphicsCommandKind::Rect
                        && command.clip != scene.screen)
                    .all(
                        |command| i32::from(command.clip.y) + i32::from(command.clip.height)
                            <= i32::from(chrome.bottom)
                    )
            );
            scene.show_details(true);
            assert!(
                scene
                    .details
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<String>()
                    .contains(&name)
            );
            all_pages(&mut scene);
            assert_eq!(scene.face(), &face);
        }
    }
}

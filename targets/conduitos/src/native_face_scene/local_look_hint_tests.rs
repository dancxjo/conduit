//! Host-local guidance preserves the admitted encounter and notice priority.
use super::*;

#[test]
fn local_look_guidance_preserves_face_hit_geometry_and_command_count() {
    let face = super::tests::face(7);
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    let (hits, commands, pages) = {
        let frame = scene.frame().unwrap();
        assert!(
            !frame
                .scene
                .commands()
                .last()
                .unwrap()
                .payload()
                .contains("F6")
        );
        (
            frame
                .hits
                .iter()
                .map(|h| (h.control, h.bounds))
                .collect::<Vec<_>>(),
            frame.scene.commands().len(),
            frame.pages,
        )
    };
    scene.set_local_creche_preferences(true);
    let frame = scene.frame().unwrap();
    assert!(
        frame
            .scene
            .commands()
            .last()
            .unwrap()
            .payload()
            .contains("F6")
    );
    assert!(
        frame
            .scene
            .commands()
            .last()
            .unwrap()
            .payload()
            .contains("Pg")
    );
    assert_eq!(
        frame
            .hits
            .iter()
            .map(|h| (h.control, h.bounds))
            .collect::<Vec<_>>(),
        hits
    );
    assert_eq!(frame.scene.commands().len(), commands);
    assert_eq!(frame.pages, pages);
    assert_eq!(scene.face(), &face);
    scene.set_local_notice("Enter value").unwrap();
    assert_eq!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .last()
            .unwrap()
            .payload(),
        "Enter value"
    );
    scene.clear_local_notice();
    scene.disable_local_input();
    assert!(
        !scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .last()
            .unwrap()
            .payload()
            .contains("F6")
    );
}

#[test]
fn local_look_guidance_selects_measured_help_without_extending_chrome() {
    for typography in [
        conduit_face::VisualTypography::Editorial,
        conduit_face::VisualTypography::Engineering,
    ] {
        for (width, height) in [(320, 240), (640, 480), (1280, 800)] {
            for scale in [100, 125, 200] {
                let screen = LayoutRect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                };
                let chrome =
                    chrome::Chrome::prepare_selected(screen, scale, Typography::new(typography))
                        .unwrap();
                let before = (chrome.top, chrome.bottom, chrome.footer);
                let hint = chrome.creche_preferences_hint(scale);
                if width >= 640 {
                    assert!(hint.is_some(), "{width} at {scale}%");
                }
                if let Some(hint) = hint {
                    assert!(hint.contains("F6") && hint.contains("F2") && hint.contains("Pg"));
                    chrome.admit_footer(hint, scale).unwrap();
                    assert!(hint.len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES);
                }
                assert_eq!((chrome.top, chrome.bottom, chrome.footer), before);
            }
        }
    }
}

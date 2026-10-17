#![cfg(feature = "native-compositor")]
extern crate alloc;
#[allow(dead_code)]
#[path = "../src/native_face_scene/fixture.rs"]
mod fixture;

use conduit_face::{
    Face, FaceRole, GraphicsCommandKind, GraphicsPaintRole, GraphicsShapeStyle, GraphicsTextRole,
};
use conduitos::native_face_scene::NativeFaceScene;

fn patchbay() -> Face {
    serde_json::from_str(include_str!(
        "../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap()
}

fn cards(scene: &NativeFaceScene) -> Vec<(i16, i16, u16, u16)> {
    scene
        .frame()
        .unwrap()
        .scene
        .commands()
        .iter()
        .filter(|command| {
            command.kind == GraphicsCommandKind::Rect
                && command.paint == GraphicsPaintRole::Accent
                && command.style == GraphicsShapeStyle::RoundedStroke
        })
        .map(|command| {
            let b = command.bounds;
            (b.x, b.y, b.width, b.height)
        })
        .collect()
}

#[test]
fn real_patchbay_face_has_compact_cards_on_large_viewport() {
    let face = patchbay();
    let retained = face.clone();
    let mut scene = NativeFaceScene::prepare(face, 1280, 720).unwrap();
    scene.show_diagram(true);
    assert_eq!(
        cards(&scene),
        vec![(32, 64, 320, 192), (368, 64, 320, 192), (32, 272, 320, 192)]
    );
    assert_eq!(scene.face(), &retained);
}

#[test]
fn narrow_page_golden_keeps_each_exact_gear_and_unicode_label_bounded() {
    let mut face = patchbay();
    let label = "é中 λ long readable identifier with a final Ω";
    for gear in face
        .subjects
        .iter_mut()
        .filter(|s| s.role == FaceRole::Gear)
    {
        gear.name = label.into();
    }
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let retained = face.clone();
    let mut scene = NativeFaceScene::prepare(face, 320, 240).unwrap();
    scene.show_diagram(true);
    assert_eq!(scene.pages(), 3);
    for _ in 0..3 {
        assert_eq!(cards(&scene), vec![(32, 64, 256, 112)]);
        let frame = scene.frame().unwrap();
        let title = frame
            .scene
            .commands()
            .iter()
            .find(|c| c.text_role() == GraphicsTextRole::Heading && c.payload().starts_with("É中"))
            .expect("one visible Gear heading");
        assert!(title.payload().ends_with("..."));
        let layout = conduitos::display::typography::TextLayout::new(
            title.payload(),
            conduitos::display::typography::TextRole::Heading,
            title.bounds.width,
        )
        .unwrap();
        assert!(layout.finish_height() <= u32::from(title.bounds.height));
        assert_eq!(title.bounds.width, 236);
        assert_eq!(scene.face(), &retained);
        scene.turn_page(true);
    }
    assert_eq!(scene.page(), 2);
}

#[test]
fn preview_uses_pinned_glyph_extents_and_retains_utf8_payload_bound() {
    use conduitos::display::typography::{TextLayout, TextRole, single_line_preview};
    for value in [
        "W".repeat(100),
        "é中 λ Ω".repeat(100),
        "e\u{301}".repeat(500),
    ] {
        for width in [36, 236, 300] {
            let preview = single_line_preview(&value, TextRole::Heading, width).unwrap();
            assert!(preview.ends_with("..."));
            assert!(preview.len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES);
            let layout = TextLayout::new(&preview, TextRole::Heading, width).unwrap();
            let glyphs = layout.collect::<Vec<_>>();
            let baseline = glyphs[0].pen.1;
            for positioned in glyphs {
                assert_eq!(positioned.pen.1, baseline, "preview must stay on one line");
                let right = positioned.pen.0
                    + i32::from(positioned.glyph.bearing().0)
                    + i32::from(positioned.glyph.extent().0);
                assert!(
                    right <= i32::from(width),
                    "visible ink exceeds the title rectangle"
                );
            }
        }
    }
    assert_eq!(
        single_line_preview("Short Ω", TextRole::Heading, 236).unwrap(),
        "Short Ω"
    );
    assert!(
        single_line_preview("A\nB", TextRole::Heading, 236)
            .unwrap()
            .ends_with("...")
    );
    assert!(single_line_preview("W", TextRole::Heading, 1).is_err());
}

#[test]
fn selected_card_opens_exact_complete_details_without_submitting_owner_action() {
    use conduitos::native_face_scene::{FaceSceneError, FaceSceneInput};
    let face = patchbay();
    let show = fixture::show(&face);
    let retained = face.clone();
    let mut scene = NativeFaceScene::prepare(face.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    assert!(matches!(
        scene.pointer(40, 70, &show, 1),
        Ok(FaceSceneInput::Changed)
    ));
    assert!(scene.showing_details());
    assert!(!scene.showing_diagram());
    assert_eq!(scene.face(), &retained);
    scene.show_diagram(true);
    assert_eq!(
        scene.inspect_subject(
            &face.identity,
            face.revision + 1,
            &face.subjects[0].identity
        ),
        Err(FaceSceneError::StaleFace)
    );
    assert_eq!(
        scene.inspect_subject(&face.identity, face.revision, "forged/subject"),
        Err(FaceSceneError::UnknownControl)
    );
    let mut stale = show;
    stale.face_revision += 1;
    assert!(matches!(
        scene.pointer(40, 70, &stale, 2),
        Err(FaceSceneError::StaleFace)
    ));
    assert!(
        scene.showing_diagram(),
        "refusal must preserve reading position"
    );
    assert_eq!(scene.face(), &retained);
}

#[test]
fn selected_long_unicode_name_survives_all_rendered_details_pages() {
    let mut face = patchbay();
    let selected = face
        .subjects
        .iter_mut()
        .filter(|s| s.role == FaceRole::Gear)
        .last()
        .unwrap();
    let identity = selected.identity.clone();
    let name = "é中 Ω complete source name ".repeat(20);
    selected.name = name.clone();
    let face = Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(face.clone(), 320, 240).unwrap();
    scene
        .inspect_subject(&face.identity, face.revision, &identity)
        .unwrap();
    let fact = format!("Gear identity: {identity}");
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|c| c.payload() == fact)
    );
    let mut visible = String::new();
    for _ in scene.page()..scene.pages() {
        let frame = scene.frame().unwrap();
        for command in frame.scene.commands().iter().filter(|c| {
            c.kind == GraphicsCommandKind::Text
                && c.text_role() == GraphicsTextRole::Body
                && c.bounds.y >= 56
                && i32::from(c.bounds.y) + i32::from(c.bounds.height) <= 184
        }) {
            visible.push_str(command.payload());
        }
        scene.turn_page(true);
    }
    assert!(
        visible.contains(&format!("Name: {name}")),
        "complete name must survive page/UTF-8 splits"
    );
    assert_eq!(scene.face(), &face);
}

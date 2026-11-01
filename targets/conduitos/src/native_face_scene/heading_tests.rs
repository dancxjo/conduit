//! Human encounter wording without guessed Body context or new authority.
use super::*;
use alloc::vec;
use conduit_face::{FaceRole, FaceSubject};

fn named_bodies(names: &[&str]) -> Face {
    let original = tests::face(8);
    Face::new_with_semantics(
        original.revision,
        original.basis.clone(),
        if names.is_empty() {
            vec![FaceSubject {
                identity: "fixture/info".into(),
                role: FaceRole::Info,
                name: "Reading".into(),
            }]
        } else {
            names
                .iter()
                .enumerate()
                .map(|(i, name)| FaceSubject {
                    identity: alloc::format!("fixture/body/{i}"),
                    role: FaceRole::Body,
                    name: (*name).into(),
                })
                .collect()
        },
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}

fn heading(scene: &NativeFaceScene) -> String {
    scene.frame().unwrap().scene.commands()[1].payload().into()
}

#[test]
fn one_explicit_body_names_both_input_modes_without_mutating_face_or_hits() {
    let face = named_bodies(&["Malo Lefèvre Laurent"]);
    for admitted in [false, true] {
        let mut scene =
            NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, admitted).unwrap();
        for details in [false, true] {
            scene.show_details(details);
            assert_eq!(heading(&scene), "Malo Lefèvre Laurent");
            assert_eq!(scene.face(), &face);
            let frame = scene.frame().unwrap();
            assert!(frame.hits.is_empty());
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            assert!(frame.scene.commands().iter().all(
                |command| command.clip_class() == conduit_face::GraphicsClipClass::FullyVisible
            ));
        }
    }
}

#[test]
fn missing_and_ambiguous_body_names_never_select_an_arbitrary_body() {
    for names in [vec![], vec!["One", "Two"]] {
        let face = named_bodies(&names);
        let mut scene =
            NativeFaceScene::prepare_with_input(face.clone(), 1024, 600, false).unwrap();
        assert_eq!(heading(&scene), "Overview");
        scene.show_details(true);
        assert_eq!(heading(&scene), "Details");
        assert_eq!(scene.face(), &face);
    }
}

#[test]
fn oversized_unicode_heading_keeps_its_complete_name_in_the_paged_document() {
    let name = "Élan ".repeat(60);
    let face = named_bodies(&[&name]);
    let mut scene = NativeFaceScene::prepare_with_input(face.clone(), 320, 240, false).unwrap();
    assert_eq!(heading(&scene), "View");
    let words: String = scene.primary.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(words, name);
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        assert!(
            frame
                .scene
                .commands()
                .iter()
                .all(|command| command.payload().len() <= conduit_face::MAX_GRAPHICS_TEXT_BYTES)
        );
        scene.turn_page(true);
    }
    assert_eq!(scene.face(), &face);
}

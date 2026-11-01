//! Typed primary content remains distinct from technical facts and disclosure.
use super::*;
use alloc::vec;
use conduit_core::{
    CheckedValueContract, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID, Quantity, Unit, kind_id,
};
use conduit_face::{FaceDisclosure, FaceProperty};

fn value_face() -> Face {
    let mut face = crate::native_face_scene::tests::face(7);
    face.properties.push(FaceProperty {
        subject: "z-first".into(),
        name: "Reading".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                kind_id(QUANTITY_INFO_ID),
                QUANTITY_ENCODED_LEN as u32,
                vec![],
            )
            .unwrap(),
            bytes: Quantity::new(42, Unit::Celsius).encode().to_vec(),
        },
    });
    reseal(face)
}
fn reseal(face: Face) -> Face {
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}
#[test]
fn primary_typed_values_use_canonical_wording_without_promoting_technical_facts() {
    let face = value_face();
    let (primary, details) = prepare(&face, false).unwrap();
    assert_eq!(
        primary
            .iter()
            .filter(|row| row.text == "Reading: 42 °C")
            .count(),
        1
    );
    assert!(
        !primary
            .iter()
            .any(|row| row.text.contains("opaque/sha256:producer"))
    );
    assert!(
        details
            .iter()
            .any(|row| row.text.contains("opaque/sha256:producer"))
    );
    assert!(primary.iter().all(|row| row.control.is_none()));
    for look in [
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
    ] {
        let admitted = conduit_face::admit_visual_look(
            &look,
            conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 200,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        let mut scene =
            NativeFaceScene::prepare_visual_look(face.clone(), 640, 480, false, &admitted).unwrap();
        let mut content = String::new();
        for _ in 0..scene.pages() {
            let frame = scene.frame().unwrap();
            assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
            for command in frame.scene.commands() {
                content.push_str(command.payload());
            }
            scene.turn_page(true);
        }
        assert!(content.contains("Reading: 42 °C"));
        assert_eq!(scene.face(), &face);
    }
}
#[test]
fn selected_detail_values_do_not_leak_into_the_primary_reading() {
    let mut face = value_face();
    face.disclosures.push(FaceDisclosure {
        subject: "z-first".into(),
        level: FaceDisclosureLevel::SelectedDetail,
    });
    let face = reseal(face);
    let (primary, details) = prepare(&face, false).unwrap();
    assert!(!primary.iter().any(|row| row.text.contains("42 °C")));
    assert!(
        details
            .iter()
            .any(|row| row.text.contains("42 degrees Celsius"))
    );
}

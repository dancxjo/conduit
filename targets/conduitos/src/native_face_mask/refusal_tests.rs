//! Actual Mask input/refusal scanout on a CPU display; not QEMU or physical proof.
use super::*;
use crate::native_face_mask::tests::{CountingDisplay, fixture};
use crate::native_face_scene::FaceFocusRequest;
use alloc::borrow::ToOwned;
use conduit_human::{KeyModifiers, KeyTransition};

fn pressed(usage: u8) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}
#[test]
fn incomplete_field_notice_preserves_show_and_source_then_real_edit_recovers() {
    let (mut mask, mut draft) = fixture();
    let basis = mask.birth_basis("arrival/refusal");
    let face = draft.face(&basis).unwrap();
    let revision = draft.revision();
    let name = draft.friendly_name().to_owned();
    let mut display = CountingDisplay::default();
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    mask.focus_named(
        &FaceFocusRequest {
            action_id: "creche.name".into(),
            argument_name: Some("value".into()),
        },
        &mut display,
    )
    .unwrap();
    let show = mask.show().unwrap().clone();
    let publication = mask.publication().unwrap().clone();
    assert!(matches!(
        mask.key(pressed(40), 1, &mut display),
        Err(NativeFaceMaskError::Scene(FaceSceneError::IncompleteInput))
    ));
    assert_eq!(mask.show(), Some(&show));
    assert_eq!(mask.publication(), Some(&publication));
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert_eq!(draft.revision(), revision);
    assert_eq!(draft.friendly_name(), name);
    assert!(
        mask.scene()
            .unwrap()
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|c| c.payload() == "Enter value")
    );
    assert_eq!(
        mask.acknowledged_input_refusal(),
        Some((FaceSceneError::IncompleteInput, "Enter value"))
    );
    assert!(mask.route_keyboard().unwrap());
    let shift = KeyEvent::new(225, KeyTransition::Pressed, KeyModifiers::LEFT_SHIFT).unwrap();
    assert!(matches!(
        mask.key(shift, 2, &mut display).unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    assert!(
        mask.scene()
            .unwrap()
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|c| c.payload() == "Enter value")
    );
    let capital = KeyEvent::new(4, KeyTransition::Pressed, KeyModifiers::LEFT_SHIFT).unwrap();
    assert!(matches!(
        mask.key(capital, 3, &mut display).unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    for (usage, sequence, modifiers) in [
        (4, 4, KeyModifiers::LEFT_SHIFT),
        (225, 5, KeyModifiers::NONE),
    ] {
        let released = KeyEvent::new(usage, KeyTransition::Released, modifiers).unwrap();
        assert!(matches!(
            mask.key(released, sequence, &mut display).unwrap(),
            NativeFaceMaskInput::Unchanged
        ));
    }
    assert!(
        !mask
            .scene()
            .unwrap()
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .any(|c| c.payload() == "Enter value")
    );
    assert!(mask.acknowledged_input_refusal().is_none());
    let NativeFaceMaskInput::Submitted { correlation, .. } =
        mask.key(pressed(40), 6, &mut display).unwrap()
    else {
        panic!("actual replacement must remain usable after refusal")
    };
    assert_eq!(correlation.interaction.arguments[0].value, b"A");
    draft
        .apply_face_interaction(&basis, &show, &correlation.interaction)
        .unwrap();
    assert_eq!(draft.revision(), revision + 1);
    assert_eq!(draft.friendly_name(), "A");
}
#[test]
fn expected_input_refusals_keep_distinct_notices_without_new_publication() {
    let (mut mask, draft) = fixture();
    let face = draft
        .face(&mask.birth_basis("arrival/refusal-table"))
        .unwrap();
    let mut display = CountingDisplay::default();
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    let show = mask.show().unwrap().clone();
    let publication = mask.publication().unwrap().clone();
    for (error, notice) in [
        (FaceSceneError::StaleFace, "Stale: refresh"),
        (FaceSceneError::UnknownControl, "Refresh"),
        (FaceSceneError::UnavailableAction, "Unavailable"),
        (FaceSceneError::UnsupportedInput, "Unsupported"),
        (FaceSceneError::InputBound, "Too long"),
        (FaceSceneError::IncompleteInput, "Enter value"),
    ] {
        assert_eq!(
            mask.present_input_result(Err(error), &mut display).err(),
            Some(NativeFaceMaskError::Scene(error))
        );
        assert_eq!(mask.acknowledged_input_refusal(), Some((error, notice)));
        assert_eq!(mask.show(), Some(&show));
        assert_eq!(mask.publication(), Some(&publication));
        assert_eq!(mask.scene().unwrap().face(), &face);
        let frame = mask.scene().unwrap().frame().unwrap();
        assert!(frame.scene.commands().iter().any(|c| c.payload() == notice));
        assert!(frame.scene.commands().len() <= conduit_face::MAX_GRAPHICS_COMMANDS);
    }
}
struct LostDisplay;
impl PixelTarget for LostDisplay {
    fn format(&self) -> crate::display::DisplayFormat {
        CountingDisplay::default().format()
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), crate::display::DisplayError> {
        Err(crate::display::DisplayError::Lost)
    }
}
#[test]
fn failed_refusal_scanout_retires_show_without_claiming_notice_visibility() {
    let (mut mask, draft) = fixture();
    let face = draft
        .face(&mask.birth_basis("arrival/refusal-display-loss"))
        .unwrap();
    let mut display = CountingDisplay::default();
    mask.present(face.clone(), 1, 1, &mut display).unwrap();
    let publication = mask.publication().unwrap().clone();
    assert!(matches!(
        mask.present_input_result(Err(FaceSceneError::IncompleteInput), &mut LostDisplay),
        Err(NativeFaceMaskError::Compositor(
            crate::native_compositor::NativeCompositorError::Display(
                crate::display::DisplayError::Lost
            )
        ))
    ));
    assert!(mask.show().is_none());
    assert!(mask.acknowledged_input_refusal().is_none());
    assert_eq!(mask.publication(), Some(&publication));
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert!(mask.route_keyboard().is_err());
}

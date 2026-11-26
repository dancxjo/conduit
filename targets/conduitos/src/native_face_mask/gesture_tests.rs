//! Synthetic physical samples through the production Mask/Fore and Birth owner.
use super::tests::{CountingDisplay, fixture};
use super::*;

pub(super) fn prepare() -> (
    NativeFaceMask,
    conduit_birth_plot::BirthDraft,
    CountingDisplay,
) {
    let (mut mask, draft) = fixture();
    let face = draft.face(&mask.birth_basis("arrival/gestures")).unwrap();
    let mut display = CountingDisplay::default();
    mask.present(face, 1, 1, &mut display).unwrap();
    // Use ordinary keyboard navigation to reveal and focus the real action.
    for sequence in 1..=64 {
        if mask
            .scene()
            .unwrap()
            .focused_request()
            .is_some_and(|focus| focus.action_id == "creche.suggest")
        {
            return (mask, draft, display);
        }
        mask.key(
            KeyEvent::new(
                43,
                KeyTransition::Pressed,
                conduit_human::KeyModifiers::NONE,
            )
            .unwrap(),
            sequence,
            &mut display,
        )
        .unwrap();
    }
    panic!("ordinary Tab navigation must reach Suggest");
}

pub(super) fn target(mask: &NativeFaceMask) -> (u32, u32) {
    let frame = mask.scene().unwrap().frame().unwrap();
    let hit = frame
        .hits
        .iter()
        .find(|hit| hit.action.identity == "creche.suggest")
        .unwrap();
    ((hit.bounds.x + 4) as u32, (hit.bounds.y + 4) as u32)
}

#[test]
fn release_and_keyboard_invoke_the_same_real_owner_action() {
    let (mut pointer, mut pointer_owner, mut display) = prepare();
    let show = pointer.show().unwrap().clone();
    let face = pointer.scene().unwrap().face().clone();
    let before = pointer.scene().unwrap().frame().unwrap().scene;
    let (x, y) = target(&pointer);
    assert!(matches!(
        pointer
            .pointer_sample(x, y, true, true, 100, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert_eq!(pointer.show(), Some(&show));
    assert_eq!(pointer.scene().unwrap().face(), &face);
    assert_ne!(
        pointer.scene().unwrap().frame().unwrap().scene,
        before,
        "pressed feedback must differ from focused feedback"
    );
    assert!(matches!(
        pointer
            .pointer_sample(x, y, true, false, 101, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    let NativeFaceMaskInput::Submitted {
        correlation: tap, ..
    } = pointer
        .pointer_sample(x, y, false, false, 102, &mut display)
        .unwrap()
    else {
        panic!("release must invoke")
    };
    assert_eq!(tap.show_id, show.show_id);
    assert!(
        pointer.show().is_none(),
        "one invocation consumes the session"
    );
    assert!(
        pointer
            .pointer_sample(x, y, false, false, 103, &mut display)
            .is_err()
    );
    let pointer_basis = pointer.birth_basis("arrival/gestures");
    pointer_owner
        .apply_face_interaction(&pointer_basis, &show, &tap.interaction)
        .unwrap();

    let (mut keyboard, mut keyboard_owner, mut keyboard_display) = prepare();
    let key_show = keyboard.show().unwrap().clone();
    let NativeFaceMaskInput::Submitted {
        correlation: key, ..
    } = keyboard
        .key(
            KeyEvent::new(
                40,
                KeyTransition::Pressed,
                conduit_human::KeyModifiers::NONE,
            )
            .unwrap(),
            102,
            &mut keyboard_display,
        )
        .unwrap()
    else {
        panic!("Enter must invoke")
    };
    assert_eq!(tap.interaction, key.interaction);
    let key_basis = keyboard.birth_basis("arrival/gestures");
    keyboard_owner
        .apply_face_interaction(&key_basis, &key_show, &key.interaction)
        .unwrap();
    assert_eq!(
        pointer_owner.face(&pointer_basis).unwrap(),
        keyboard_owner.face(&key_basis).unwrap()
    );
}

#[test]
fn movement_leave_loss_and_show_replacement_cannot_commit_a_press() {
    for cancellation in 0..7 {
        let (mut mask, _, mut display) = prepare();
        let (x, y) = target(&mask);
        let show = mask.show().unwrap().clone();
        mask.pointer_sample(x, y, true, true, 100, &mut display)
            .unwrap();
        match cancellation {
            0 => {
                mask.pointer_sample(x + 9, y, true, false, 101, &mut display)
                    .unwrap();
            }
            1 => {
                mask.pointer_sample(639, 479, true, false, 101, &mut display)
                    .unwrap();
            }
            2 => {
                mask.cancel_pointer_gesture(&mut display).unwrap();
            }
            3 => {
                let face = mask.scene().unwrap().face().clone();
                // Renewal retires the existing surface before acknowledging a
                // different Show of the same immutable Face.
                mask.suspend().unwrap();
                mask.present(face, 2, 2, &mut display).unwrap();
            }
            4 => {
                let scene = mask.scene.as_mut().unwrap();
                scene.show_details(true);
                scene.show_details(false);
                mask.repaint(&mut display).unwrap();
            }
            5 => {
                let request = mask.scene().unwrap().focused_request().unwrap();
                mask.focus_named(&request, &mut display).unwrap();
            }
            _ => {
                mask.scene
                    .as_mut()
                    .unwrap()
                    .key(
                        KeyEvent::new(
                            43,
                            KeyTransition::Pressed,
                            conduit_human::KeyModifiers::NONE,
                        )
                        .unwrap(),
                        &show,
                        101,
                    )
                    .unwrap();
                mask.repaint(&mut display).unwrap();
            }
        }
        // Returning to the target while held cannot acquire another gesture.
        mask.pointer_sample(x, y, true, false, 102, &mut display)
            .unwrap();
        assert!(matches!(
            mask.pointer_sample(x, y, false, false, 103, &mut display)
                .unwrap(),
            NativeFaceMaskInput::Unchanged
        ));
        assert!(mask.show().is_some());
        if cancellation != 3 {
            assert_eq!(mask.show(), Some(&show));
        }
    }
}

#[test]
fn keyboard_navigation_cancels_pending_pointer_ownership() {
    let (mut mask, _, mut display) = prepare();
    let (x, y) = target(&mask);
    mask.pointer_sample(x, y, true, true, 100, &mut display)
        .unwrap();
    mask.key(
        KeyEvent::new(
            43,
            KeyTransition::Pressed,
            conduit_human::KeyModifiers::NONE,
        )
        .unwrap(),
        101,
        &mut display,
    )
    .unwrap();
    assert!(matches!(
        mask.pointer_sample(x, y, false, false, 102, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    assert!(mask.show().is_some());
}

#[test]
fn provisional_press_preserves_focus_selection_and_frame_budget() {
    let (mut mask, _, mut display) = prepare();
    let focus = mask.scene().unwrap().focused();
    let face = mask.scene().unwrap().face().clone();
    let before = mask.scene().unwrap().frame().unwrap();
    let count = before.scene.commands().len();
    let hit = before
        .hits
        .iter()
        .find(|hit| Some(hit.control) != focus)
        .unwrap();
    let (x, y) = ((hit.bounds.x + 4) as u32, (hit.bounds.y + 4) as u32);
    let before_scene = before.scene;
    drop(before);
    mask.pointer_sample(x, y, true, true, 100, &mut display)
        .unwrap();
    let pending = mask.scene().unwrap();
    assert_eq!(pending.focused(), focus);
    assert_eq!(pending.face(), &face);
    assert!(pending.frame().unwrap().scene.commands().len() <= count);
    mask.cancel_pointer_gesture(&mut display).unwrap();
    assert_eq!(mask.scene().unwrap().focused(), focus);
    assert_eq!(mask.scene().unwrap().frame().unwrap().scene, before_scene);
}
#[test]
fn sampled_footer_page_repaints_without_consuming_the_current_show() {
    let (mut mask, _, mut display) = prepare();
    let show = mask.show().unwrap().clone();
    let face = mask.scene().unwrap().face().clone();
    let page = mask.scene().unwrap().page();
    let controls = mask.scene().unwrap().page_controls().unwrap().unwrap();
    let (bounds, expected) = if controls.can_next {
        (controls.next, page + 1)
    } else {
        assert!(controls.can_previous);
        (controls.previous, page - 1)
    };
    let x = (bounds.x + (bounds.width / 2) as i16) as u32;
    let y = (bounds.y + (bounds.height / 2) as i16) as u32;
    assert!(matches!(
        mask.pointer_sample(x, y, true, true, 200, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert_eq!(mask.scene().unwrap().page(), page);
    assert_eq!(mask.show(), Some(&show));
    assert!(matches!(
        mask.pointer_sample(x, y, true, false, 201, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    assert!(matches!(
        mask.pointer_sample(x, y, false, false, 202, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Redrawn(_)
    ));
    assert_eq!(mask.scene().unwrap().page(), expected);
    assert_eq!(mask.show(), Some(&show));
    assert_eq!(mask.scene().unwrap().face(), &face);
    assert!(matches!(
        mask.pointer_sample(x, y, false, false, 203, &mut display)
            .unwrap(),
        NativeFaceMaskInput::Unchanged
    ));
    assert_eq!(mask.scene().unwrap().page(), expected);
}

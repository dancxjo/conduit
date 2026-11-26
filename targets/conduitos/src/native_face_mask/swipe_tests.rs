//! Ordered samples through the real Mask Play: reading never invokes the owner.
use super::gesture_tests::{prepare, target};
use super::*;

#[test]
fn swipe_pages_both_ways_once_on_release_without_consuming_show() {
    let (mut mask, _, mut display) = prepare();
    while mask.scene().unwrap().page() > 0 {
        mask.scene.as_mut().unwrap().turn_page(false);
    }
    mask.repaint(&mut display).unwrap();
    let show = mask.show().unwrap().clone();
    let face = mask.scene().unwrap().face().clone();
    for (start, end, page) in [(400, 150, 1), (150, 400, 0), (150, 400, 0)] {
        mask.pointer_sample(start, 100, true, true, 100, &mut display)
            .unwrap();
        mask.pointer_sample(end, 100, true, false, 101, &mut display)
            .unwrap();
        let before = mask.scene().unwrap().page();
        let result = mask
            .pointer_sample(end, 100, false, false, 102, &mut display)
            .unwrap();
        assert!(!matches!(result, NativeFaceMaskInput::Submitted { .. }));
        assert_eq!(mask.scene().unwrap().page(), page);
        if before != page {
            assert!(matches!(result, NativeFaceMaskInput::Redrawn(_)));
        }
        mask.pointer_sample(end, 100, false, false, 103, &mut display)
            .unwrap();
        assert_eq!(mask.scene().unwrap().page(), page);
        assert_eq!(mask.show(), Some(&show));
        assert_eq!(mask.scene().unwrap().face(), &face);
    }
}

#[test]
fn swipe_from_an_action_cannot_invoke_it_and_vertical_motion_cannot_become_a_swipe() {
    for vertical in [0, 25] {
        let (mut mask, _, mut display) = prepare();
        let (x, y) = target(&mask);
        let page = mask.scene().unwrap().page();
        let show = mask.show().unwrap().clone();
        let end = if x > 250 { x - 200 } else { x + 200 };
        mask.pointer_sample(x, y, true, true, 100, &mut display)
            .unwrap();
        mask.pointer_sample(end, y + vertical, true, false, 101, &mut display)
            .unwrap();
        // Returning vertically to the origin cannot restore swipe eligibility.
        let result = mask
            .pointer_sample(end, y, false, false, 102, &mut display)
            .unwrap();
        assert!(!matches!(result, NativeFaceMaskInput::Submitted { .. }));
        assert_eq!(mask.show(), Some(&show));
        if vertical != 0 {
            assert_eq!(mask.scene().unwrap().page(), page);
        }
    }
}

#[test]
fn cancelled_or_replaced_show_does_not_rearm_a_held_swipe() {
    for replace in [false, true] {
        let (mut mask, _, mut display) = prepare();
        let page = mask.scene().unwrap().page();
        mask.pointer_sample(400, 100, true, true, 100, &mut display)
            .unwrap();
        if replace {
            let face = mask.scene().unwrap().face().clone();
            mask.suspend().unwrap();
            mask.present(face, 2, 2, &mut display).unwrap();
        } else {
            mask.cancel_pointer_gesture(&mut display).unwrap();
        }
        let expected = if replace {
            mask.scene().unwrap().page()
        } else {
            page
        };
        mask.pointer_sample(150, 100, true, false, 101, &mut display)
            .unwrap();
        let result = mask
            .pointer_sample(150, 100, false, false, 102, &mut display)
            .unwrap();
        assert!(matches!(result, NativeFaceMaskInput::Unchanged));
        assert_eq!(mask.scene().unwrap().page(), expected);
    }
}

#[test]
fn short_motion_and_last_page_do_not_turn_or_submit() {
    let (mut mask, _, mut display) = prepare();
    let page = mask.scene().unwrap().page();
    mask.pointer_sample(400, 100, true, true, 100, &mut display)
        .unwrap();
    let result = mask
        .pointer_sample(391, 100, false, false, 101, &mut display)
        .unwrap();
    assert!(!matches!(result, NativeFaceMaskInput::Submitted { .. }));
    assert_eq!(mask.scene().unwrap().page(), page);
    while mask.scene().unwrap().page() + 1 < mask.scene().unwrap().pages() {
        mask.scene.as_mut().unwrap().turn_page(true);
    }
    mask.repaint(&mut display).unwrap();
    let page = mask.scene().unwrap().page();
    let show = mask.show().unwrap().clone();
    mask.pointer_sample(400, 100, true, true, 102, &mut display)
        .unwrap();
    mask.pointer_sample(150, 100, false, false, 103, &mut display)
        .unwrap();
    assert_eq!(mask.scene().unwrap().page(), page);
    assert_eq!(mask.show(), Some(&show));
}

use super::*;
use std::vec;

#[test]
fn active_frame_plan_is_exact_clipped_and_stable_through_partial_progress() {
    let (mut a, mut b) = (vec![0; MAX_PIXELS], vec![0; MAX_PIXELS]);
    let mut c = Compositor::new(&mut a, &mut b, 20, 12).unwrap();
    c.damage.add(Rect {
        x: -2,
        y: -3,
        width: 5,
        height: 5,
    });
    c.damage.add(Rect {
        x: 8,
        y: 7,
        width: 30,
        height: 30,
    });
    let sequence = c.begin_frame().unwrap();
    let plan = c.frame_plan().unwrap();
    assert_eq!(plan.sequence, sequence);
    assert_eq!(plan.count, 2);
    assert_eq!(
        plan.rects[0],
        FrameRect {
            x: 0,
            y: 0,
            width: 3,
            height: 2
        }
    );
    assert_eq!(
        plan.rects[1],
        FrameRect {
            x: 8,
            y: 7,
            width: 12,
            height: 5
        }
    );
    assert!(plan.rects[2..].iter().all(|r| *r == FrameRect::default()));
    assert!(!plan.conservative_fallback && !plan.cursor_visible && !plan.focus_visible);
    let first = c.compose_next(sequence, &mut [0; 1]).unwrap();
    assert!(!first.complete);
    assert_eq!(first.selected_mask, 0);
    assert_eq!(c.frame_plan(), Some(plan));
    while !c.compose_next(sequence, &mut [0; 16]).unwrap().complete {}
    assert_eq!(c.frame_plan(), None);
}

#[test]
fn selected_surface_can_have_zero_coverage_under_cursor_ink() {
    let (mut a, mut b) = (vec![0; MAX_PIXELS], vec![0; MAX_PIXELS]);
    let mut c = Compositor::new(&mut a, &mut b, 1, 1).unwrap();
    let rect = Rect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    let h = c.admit(rect, 0).unwrap();
    c.begin_upload(h, 1, rect).unwrap();
    assert!(c.prepare_upload(1).unwrap());
    c.upload_chunk(0, &[0x123456]).unwrap();
    c.commit_upload().unwrap();
    c.cursor(Some((0, 0)), false).unwrap();
    let sequence = c.begin_frame().unwrap();
    assert!(c.frame_plan().unwrap().cursor_visible);
    let mut output = [u32::MAX; 1];
    let span = c.compose_next(sequence, &mut output).unwrap();
    assert!(span.complete);
    assert_eq!(output, [0]);
    assert_eq!(span.selected_mask, 1 << h.slot);
    let done = c.completed().unwrap();
    assert_eq!(done.selected_mask(), 1 << h.slot);
    assert_eq!(done.coverage(), [0; MAX_SURFACES]);
}

#[test]
fn damage_capacity_fallback_and_logical_affordance_flags_survive_clipping() {
    let (mut a, mut b) = (vec![0; MAX_PIXELS], vec![0; MAX_PIXELS]);
    let mut c = Compositor::new(&mut a, &mut b, 40, 2).unwrap();
    for i in 0..17 {
        c.damage.add(Rect {
            x: i * 3,
            y: 0,
            width: 1,
            height: 1,
        });
    }
    // Preserve native logical presence even when the cursor is outside view.
    c.cursor = Some((100, 100));
    c.focus = Some(Handle {
        slot: 7,
        generation: 1,
    });
    c.begin_frame().unwrap();
    let plan = c.frame_plan().unwrap();
    assert_eq!(plan.count, 1);
    assert_eq!(
        plan.rects[0],
        FrameRect {
            x: 0,
            y: 0,
            width: 40,
            height: 1
        }
    );
    assert!(plan.conservative_fallback && plan.cursor_visible && plan.focus_visible);
}

#[test]
fn idle_frame_retains_guest_logical_affordances_without_new_pixel_work() {
    let (mut a, mut b) = (vec![0; MAX_PIXELS], vec![0; MAX_PIXELS]);
    let mut c = Compositor::new(&mut a, &mut b, 1, 1).unwrap();
    assert_eq!(c.begin_frame(), Err(Error::NoDamage));
    assert_eq!(c.logical_affordances(), (false, false));
    c.cursor(Some((100, 100)), false).unwrap();
    assert_eq!(c.begin_frame(), Err(Error::NoDamage));
    assert_eq!(c.logical_affordances(), (true, false));
    assert_eq!(c.frame_plan(), None);
}

use super::{arenas, render, upload};
use crate::*;
use std::vec;

#[test]
fn focus_before_first_scanout_draws_but_cannot_route_input() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 8, 8).unwrap();
    let rect = Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    };
    let h = c.admit(rect, 0).unwrap();
    assert_eq!(c.focus(h), Err(Error::Incomplete));
    upload(&mut c, h, 1, rect, &[9; 64]);
    c.focus(h).unwrap();
    assert_eq!(c.pointer(2, 2), None);
    assert_eq!(c.keyboard(), None);
    let mut screen = [0; 64];
    let frame = render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(screen[0], 0x00ffcc33);
    assert_eq!(screen[9], 9);
    assert_eq!(c.keyboard(), None);
    c.acknowledge_scanout(frame.sequence()).unwrap();
    assert_eq!(c.keyboard(), Some((h, 1)));
    let frame_before = c.completed();
    c.focus(h).unwrap();
    c.resize(h, rect, 0).unwrap();
    assert_eq!(c.completed(), frame_before);
    assert_eq!(c.keyboard(), Some((h, 1)));
    c.visible(h, false).unwrap();
    assert_eq!(c.focus(h), Err(Error::Incomplete));
}

#[test]
fn resize_retires_old_raster_blocks_input_and_preserves_other_packed_rasters() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 8, 8).unwrap();
    let whole = Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    };
    let lower = c.admit(whole, 0).unwrap();
    upload(&mut c, lower, 1, whole, &[3; 64]);
    let old = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let top = c.admit(old, 1).unwrap();
    upload(&mut c, top, 7, old, &[9; 16]);
    let mut screen = [0; 64];
    let frame = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(frame.sequence()).unwrap();
    assert_eq!(c.pointer(1, 1).unwrap().surface, top);
    let resized = Rect {
        width: 6,
        height: 6,
        ..old
    };
    c.resize(top, resized, 2).unwrap();
    assert_eq!(c.completed(), None);
    assert_eq!(c.surfaces[1].revision, 7);
    assert!(!c.surfaces[1].ready);
    assert!(c.acknowledged[0]);
    assert_eq!(c.pointer(1, 1), None);
    assert_eq!(c.pointer(5, 5), None);
    assert_eq!(c.begin_upload(top, 7, resized), Err(Error::Stale));
    let cleared = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(cleared.sequence()).unwrap();
    assert_eq!(screen, [3; 64]);
    assert_eq!(c.pointer(1, 1), None);
    // Force a different surface's transaction to repack while this slot is pending.
    upload(&mut c, lower, 2, whole, &[4; 64]);
    let cleared = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(cleared.sequence()).unwrap();
    assert_eq!(screen, [4; 64]);
    assert_eq!(c.pointer(1, 1), None);
    upload(&mut c, top, 8, resized, &[8; 36]);
    c.focus(top).unwrap();
    assert_eq!(c.keyboard(), None);
    assert_eq!(c.pointer(1, 1), None);
    let updated = render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(screen[9], 8);
    assert_eq!(screen[7 * 8 + 7], 4);
    c.acknowledge_scanout(updated.sequence()).unwrap();
    assert_eq!(c.keyboard(), Some((top, 8)));
    assert_eq!(c.pointer(1, 1).unwrap().surface, top);
}

#[test]
fn invalid_resize_keeps_pending_frame_tokens_and_pixel_reservations_unchanged() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 8, 8).unwrap();
    let small = Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    };
    let h = c.admit(small, 0).unwrap();
    upload(&mut c, h, 1, small, &[5; 64]);
    let pending = Rect {
        x: 0,
        y: 0,
        width: 1024,
        height: 1024,
    };
    c.admit(pending, 1).unwrap();
    let mut screen = [0; 64];
    let frame = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(frame.sequence()).unwrap();
    c.cursor(Some((7, 7)), false).unwrap();
    let active = c.begin_frame().unwrap();
    let too_large = Rect {
        width: 2048,
        height: 1024,
        ..small
    };
    assert_eq!(c.resize(h, too_large, 3), Err(Error::Capacity));
    assert_eq!(
        c.resize(h, Rect { width: 0, ..small }, 3),
        Err(Error::Bounds)
    );
    assert_eq!(c.surfaces[0].rect, small);
    assert_eq!(c.surfaces[0].z, 0);
    assert!(c.surfaces[0].ready);
    assert!(c.acknowledged[0]);
    let mut output = [0; 16];
    let step = c.compose_next(active, &mut output).unwrap();
    assert!(step.complete);
    let completed = c.completed();
    c.resize(h, small, 0).unwrap();
    assert_eq!(c.completed(), completed);
    // A pending resized extent participates in subsequent admission limits.
    c.resize(
        h,
        Rect {
            width: 1024,
            height: 1024,
            ..small
        },
        0,
    )
    .unwrap();
    assert_eq!(
        c.admit(
            Rect {
                width: 1,
                height: 1,
                ..small
            },
            0
        ),
        Err(Error::Capacity)
    );
    // Invalid paths never replace either caller-owned arena.
    assert_eq!(c.pixels[0], 5);
}

#[test]
fn resize_equal_dimensions_is_a_move_and_keeps_the_current_raster() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 8, 8).unwrap();
    let rect = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let h = c.admit(rect, 0).unwrap();
    upload(&mut c, h, 1, rect, &[6; 16]);
    let mut screen = vec![0; 64];
    let frame = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(frame.sequence()).unwrap();
    c.resize(h, Rect { x: 4, y: 4, ..rect }, 1).unwrap();
    assert!(c.surfaces[0].ready);
    assert!(c.acknowledged[0]);
    render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(screen[0], 0);
    assert_eq!(screen[4 * 8 + 4], 6);
}

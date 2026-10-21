use super::*;
use std::{vec, vec::Vec};
fn arenas() -> (Vec<u32>, Vec<u32>) {
    (vec![0; MAX_PIXELS], vec![0; MAX_PIXELS])
}
fn upload(c: &mut Compositor<'_>, h: Handle, rev: u64, r: Rect, pixels: &[u32]) {
    c.begin_upload(h, rev, r).unwrap();
    while !c.prepare_upload(MAX_CHUNK).unwrap() {}
    for (i, chunk) in pixels.chunks(MAX_CHUNK).enumerate() {
        c.upload_chunk(i * MAX_CHUNK, chunk).unwrap();
    }
    c.commit_upload().unwrap();
}
fn render(c: &mut Compositor<'_>, screen: &mut [u32], chunk: usize) -> CompletedFrame {
    let seq = c.begin_frame().unwrap();
    let mut out = vec![0; chunk];
    loop {
        let span = c.compose_next(seq, &mut out).unwrap();
        let start = span.y as usize * usize::from(c.width) + span.x as usize;
        screen[start..start + span.pixels].copy_from_slice(&out[..span.pixels]);
        if span.complete {
            break;
        }
    }
    c.completed().unwrap()
}

#[test]
fn full_capacity_variable_surfaces_resize_and_compaction_are_transactional() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 32, 32).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 2048,
        height: 1024,
    };
    let h = c.admit(r, 0).unwrap();
    upload(&mut c, h, 1, r, &vec![11; MAX_PIXELS]);
    assert_eq!(c.pixels[MAX_PIXELS - 1], 11);
    let r2 = Rect { width: 1024, ..r };
    upload(&mut c, h, 2, r2, &vec![12; MAX_PIXELS / 2]);
    let h2 = c.admit(r2, 1).unwrap();
    upload(&mut c, h2, 1, r2, &vec![13; MAX_PIXELS / 2]);
    assert_eq!(c.surfaces[1].offset, MAX_PIXELS / 2);
    c.remove(h).unwrap();
    let h3 = c.admit(r2, 1).unwrap();
    assert_ne!(h, h3);
    assert_eq!(c.remove(h), Err(Error::Stale));
    upload(&mut c, h3, 1, r2, &vec![14; MAX_PIXELS / 2]);
    assert_eq!(c.pixels[0], 14);
    assert_eq!(c.pixels[MAX_PIXELS - 1], 13);
    // Slot reuse must still sort as the latest surface for equal z.
    assert_eq!(c.selected(1, 1), Some(usize::from(h3.slot)));
    assert!(core::mem::size_of::<Compositor<'_>>() < 256 * 1024);
}
#[test]
fn malformed_stale_partial_upload_and_failed_frame_cannot_authorize_input() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 8, 8).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    };
    let h = c.admit(r, 0).unwrap();
    assert_eq!(c.begin_upload(h, 0, r), Err(Error::Stale));
    assert_eq!(
        c.begin_upload(h, 1, Rect { width: 0, ..r }),
        Err(Error::Bounds)
    );
    upload(&mut c, h, 1, r, &[7; 64]);
    let mut screen = [0; 64];
    let first = render(&mut c, &mut screen, 3);
    assert_eq!(c.pointer(2, 2), None);
    c.acknowledge_scanout(first.sequence()).unwrap();
    assert!(c.pointer(2, 2).is_some());
    c.begin_upload(h, 2, r).unwrap();
    assert_eq!(c.pointer(2, 2), None);
    assert_eq!(c.upload_chunk(0, &[8]), Err(Error::Order));
    assert_eq!(c.commit_upload(), Err(Error::Incomplete));
    assert_eq!(c.begin_frame(), Err(Error::Busy));
    while !c.prepare_upload(1).unwrap() {}
    assert_eq!(c.upload_chunk(1, &[8]), Err(Error::Order));
    assert_eq!(c.upload_chunk(0, &[8; 65]), Err(Error::Bounds));
    c.upload_chunk(0, &[8; 32]).unwrap();
    assert_eq!(c.commit_upload(), Err(Error::Incomplete));
    assert_eq!(c.pixels[0], 7);
    c.abort_upload().unwrap();
    assert_eq!(c.pixels[0], 7);
    assert_eq!(c.acknowledge_scanout(first.sequence()), Err(Error::Stale));
    let seq = c.begin_frame().unwrap();
    let mut out = [0; 1];
    c.compose_next(seq, &mut out).unwrap();
    assert_eq!(c.acknowledge_scanout(seq), Err(Error::Stale));
    assert_eq!(c.compose_next(seq, &mut []), Err(Error::Bounds));
    assert_eq!(c.pointer(2, 2), None);
    let done = render(&mut c, &mut screen, MAX_CHUNK);
    c.fail_frame();
    assert_eq!(c.acknowledge_scanout(done.sequence()), Err(Error::Stale));
    let done = render(&mut c, &mut screen, 2);
    c.acknowledge_scanout(done.sequence()).unwrap();
    assert_eq!(screen, [7; 64]);
    assert!(c.pointer(2, 2).is_some());
    assert_eq!(c.begin_upload(h, 1, r), Err(Error::Stale));
    assert_eq!(c.cursor(Some((u32::MAX, 0)), false), Err(Error::Bounds));
}
#[test]
fn frozen_native_reference_matches_overlaps_clipping_ties_cursor_and_focus() {
    for seed in 0..16u32 {
        let (mut a, mut b) = arenas();
        let mut c = Compositor::new(&mut a, &mut b, 31, 25).unwrap();
        let mut fixtures = Vec::new();
        for i in 0..8 {
            let r = Rect {
                x: ((seed * 7 + i * 5) % 29) as i16 - 8,
                y: ((seed * 3 + i * 7) % 23) as i16 - 6,
                width: (4 + (seed + i * 3) % 14) as u16,
                height: (4 + (seed + i * 5) % 17) as u16,
            };
            let pixels = (0..r.pixels().unwrap())
                .map(|j| 0x00123400 + i * 19 + j as u32)
                .collect::<Vec<_>>();
            let h = c.admit(r, (i % 3) as u8).unwrap();
            upload(&mut c, h, 1, r, &pixels);
            fixtures.push(reference::Surface {
                rect: r,
                z: (i % 3) as u8,
                visible: i != 3,
                pixels,
            });
            if i == 3 {
                c.visible(h, false).unwrap();
            }
        }
        let mut screen = vec![0; 31 * 25];
        let initial = render(&mut c, &mut screen, MAX_CHUNK);
        c.acknowledge_scanout(initial.sequence()).unwrap();
        let focus = initial
            .coverage()
            .iter()
            .position(|&count| count > 0)
            .unwrap();
        c.focus(Handle {
            slot: focus as u8,
            generation: 1,
        })
        .unwrap();
        let cursor = (seed as u16, seed as u16 / 2);
        c.cursor(
            Some((u32::from(cursor.0), u32::from(cursor.1))),
            seed % 2 == 0,
        )
        .unwrap();
        c.fail_frame();
        let done = render(&mut c, &mut screen, 1 + (seed as usize * 71) % MAX_CHUNK);
        let (expected, coverage) =
            reference::compose(&fixtures, 31, 25, Some(focus), Some(cursor), seed % 2 == 0);
        assert_eq!(screen, expected, "seed {seed}");
        assert_eq!(done.coverage(), coverage, "coverage seed {seed}");
    }
}
#[test]
fn lifecycle_damage_repaints_old_bounds_and_ack_is_invalidated() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 9, 9).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let h = c.admit(r, 0).unwrap();
    upload(&mut c, h, 1, r, &[9; 16]);
    let mut screen = [0; 81];
    let done = render(&mut c, &mut screen, 3);
    c.acknowledge_scanout(done.sequence()).unwrap();
    c.focus(h).unwrap();
    assert_eq!(c.keyboard(), Some((h, 1)));
    let done = render(&mut c, &mut screen, 7);
    c.acknowledge_scanout(done.sequence()).unwrap();
    assert_eq!(c.keyboard(), Some((h, 1)));
    c.place(h, 4, 4, 2).unwrap();
    assert_eq!(c.pointer(1, 1), None);
    render(&mut c, &mut screen, 3);
    assert_eq!(screen[0], 0);
    assert_eq!(screen[4 * 9 + 4], 0x00ffcc33);
    c.visible(h, false).unwrap();
    render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(screen, [0; 81]);
    c.remove(h).unwrap();
    assert_eq!(c.focus(h), Err(Error::Stale));
}
mod reference;

#[test]
fn composition_steps_enforce_64kib_limit_and_stale_continuations() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 32768, 1).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 32768,
        height: 1,
    };
    let h = c.admit(r, 0).unwrap();
    upload(&mut c, h, 1, r, &vec![3; 32768]);
    let seq = c.begin_frame().unwrap();
    let mut output = vec![0; MAX_CHUNK];
    let first = c.compose_next(seq, &mut output).unwrap();
    assert_eq!(first.pixels, MAX_CHUNK);
    assert!(!first.complete);
    assert_eq!(first.x, 0);
    assert_eq!(c.pointer(0, 0), None);
    let second = c.compose_next(seq, &mut output).unwrap();
    assert_eq!(second.x, MAX_CHUNK as u32);
    assert!(second.complete);
    assert_eq!(c.completed().unwrap().coverage()[0], 32768);
    c.acknowledge_scanout(seq).unwrap();
    assert!(c.pointer(0, 0).is_some());
    c.place(h, 0, 0, 1).unwrap();
    let next = c.begin_frame().unwrap();
    assert!(next > seq);
    assert_eq!(c.compose_next(seq, &mut output), Err(Error::Stale));
    assert_eq!(
        c.compose_next(next, &mut vec![0; MAX_CHUNK + 1]),
        Err(Error::Bounds)
    );
    assert_eq!(c.completed(), None);
    assert_eq!(c.acknowledge_scanout(seq), Err(Error::Stale));
}
#[test]
fn damage_overflow_conservatively_unions_and_capacity_refuses() {
    let mut damage = Damage::new();
    for x in 0..17 {
        damage.add(Rect {
            x: x * 3,
            y: 0,
            width: 1,
            height: 1,
        });
    }
    assert!(damage.fallback);
    assert_eq!(damage.count, 1);
    assert_eq!(damage.rects[0].left, 0);
    assert_eq!(damage.rects[0].right, 49);
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 1, 1).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
    for _ in 0..MAX_SURFACES {
        c.admit(r, 0).unwrap();
    }
    assert_eq!(c.admit(r, 0), Err(Error::Capacity));
    let h = Handle {
        slot: 0,
        generation: 1,
    };
    assert_eq!(
        c.begin_upload(
            h,
            1,
            Rect {
                width: u16::MAX,
                height: u16::MAX,
                ..r
            }
        ),
        Err(Error::Capacity)
    );
}
#[test]
fn pending_top_surface_blocks_pointer_fallthrough() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 4, 4).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let lower = c.admit(r, 0).unwrap();
    upload(&mut c, lower, 1, r, &[3; 16]);
    let top = c.admit(r, 1).unwrap();
    let mut screen = [0; 16];
    let frame = render(&mut c, &mut screen, 4);
    c.acknowledge_scanout(frame.sequence()).unwrap();
    assert_eq!(c.pointer(1, 1), None);
    c.visible(top, false).unwrap();
    let frame = render(&mut c, &mut screen, 4);
    c.acknowledge_scanout(frame.sequence()).unwrap();
    assert_eq!(c.pointer(1, 1).unwrap().surface, lower);
}

#[test]
fn unchanged_input_readiness_survives_overlay_partial_and_no_damage_frames() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 64, 32).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    };
    let h = c.admit(r, 0).unwrap();
    upload(&mut c, h, 1, r, &[7; 64]);
    assert_eq!(c.focus(h), Err(Error::Incomplete));
    let mut screen = vec![0; 64 * 32];
    let initial = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(initial.sequence()).unwrap();
    c.focus(h).unwrap();
    let focused = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(focused.sequence()).unwrap();
    c.cursor(Some((40, 10)), true).unwrap();
    assert_eq!(c.keyboard(), Some((h, 1)));
    assert!(c.pointer(2, 2).is_some());
    let cursor = render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(cursor.coverage(), [0; 8]);
    c.acknowledge_scanout(cursor.sequence()).unwrap();
    assert_eq!(c.keyboard(), Some((h, 1)));
    c.place(h, 0, 0, 0).unwrap();
    c.visible(h, true).unwrap();
    c.focus(h).unwrap();
    c.cursor(Some((40, 10)), true).unwrap();
    assert_eq!(c.completed(), Some(cursor));
    assert_eq!(c.begin_frame(), Err(Error::NoDamage));
    assert_eq!(c.keyboard(), Some((h, 1)));
    assert!(c.pointer(2, 2).is_some());
    let other_rect = Rect {
        x: 20,
        y: 0,
        width: 4,
        height: 4,
    };
    let other = c.admit(other_rect, 1).unwrap();
    upload(&mut c, other, 1, other_rect, &[9; 16]);
    let partial = render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(partial.coverage()[0], 0);
    c.acknowledge_scanout(partial.sequence()).unwrap();
    assert_eq!(c.keyboard(), Some((h, 1)));
    c.begin_upload(h, 2, r).unwrap();
    assert_eq!(c.keyboard(), None);
    assert_eq!(c.pointer(2, 2), None);
    assert_eq!(c.pointer(21, 1).unwrap().surface, other);
    while !c.prepare_upload(MAX_CHUNK).unwrap() {}
    c.upload_chunk(0, &[8; 64]).unwrap();
    c.commit_upload().unwrap();
    assert_eq!(c.keyboard(), None);
    let updated = render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(c.keyboard(), None);
    c.acknowledge_scanout(updated.sequence()).unwrap();
    assert_eq!(c.keyboard(), Some((h, 2)));
    c.fail_frame();
    assert_eq!(c.keyboard(), None);
    assert_eq!(c.pointer(21, 1), None);
}
#[test]
fn admission_and_resize_reserve_pending_extents_before_effects() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, 4, 4).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: 4,
        height: 4,
    };
    let h = c.admit(r, 0).unwrap();
    upload(&mut c, h, 1, r, &[1; 16]);
    let mut screen = [0; 16];
    let ack = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(ack.sequence()).unwrap();
    let whole = Rect {
        width: 2048,
        height: 1024,
        ..r
    };
    assert_eq!(c.admit(whole, 1), Err(Error::Capacity));
    assert_eq!(c.completed(), Some(ack));
    assert_eq!(c.pointer(1, 1).unwrap().surface, h);
    let pending = c
        .admit(
            Rect {
                width: 1024,
                height: 1024,
                ..r
            },
            1,
        )
        .unwrap();
    assert_eq!(c.begin_upload(h, 2, whole), Err(Error::Capacity));
    assert!(c.upload.is_none());
    assert!(c.acknowledged[0]);
    assert_eq!(c.surfaces[0].revision, 1);
    assert_eq!(
        c.admit(
            Rect {
                width: 1024,
                height: 1024,
                ..r
            },
            2
        ),
        Err(Error::Capacity)
    );
    assert_eq!(c.focus(pending), Err(Error::Incomplete));
    c.visible(pending, false).unwrap();
    c.focus(h).unwrap();
    c.visible(h, false).unwrap();
    assert_eq!(c.focus, None);
    assert_eq!(c.focus(h), Err(Error::Incomplete));
}
#[test]
fn cursor_uses_full_display_extent_and_checked_i32_damage_coordinates() {
    let (mut a, mut b) = arenas();
    let mut c = Compositor::new(&mut a, &mut b, u16::MAX, 1).unwrap();
    let r = Rect {
        x: 0,
        y: 0,
        width: u16::MAX,
        height: 1,
    };
    let h = c.admit(r, 0).unwrap();
    upload(&mut c, h, 1, r, &vec![5; usize::from(u16::MAX)]);
    let mut screen = vec![0; usize::from(u16::MAX)];
    let initial = render(&mut c, &mut screen, MAX_CHUNK);
    c.acknowledge_scanout(initial.sequence()).unwrap();
    c.cursor(Some((65534, 0)), false).unwrap();
    let edge = render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(screen[65534], 0);
    c.acknowledge_scanout(edge.sequence()).unwrap();
    assert!(c.pointer(1, 0).is_some());
    c.cursor(Some((100_000, 100_000)), false).unwrap();
    render(&mut c, &mut screen, MAX_CHUNK);
    assert_eq!(screen[65534], 5);
    assert_eq!(c.cursor, Some((100_000, 100_000)));
    let old = c.cursor;
    assert_eq!(
        c.cursor(Some((i32::MAX as u32, 0)), false),
        Err(Error::Bounds)
    );
    assert_eq!(c.cursor, old);
}

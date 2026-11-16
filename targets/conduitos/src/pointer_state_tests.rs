//! Physical button history is independent of semantic publication identity.
use super::*;
fn sample(sequence: u64, primary_pressed: bool) -> NormalizedPointerSample {
    NormalizedPointerSample {
        position_x: 500_000,
        position_y: 500_000,
        delta_x: 0,
        delta_y: 0,
        primary_pressed,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 2,
        sequence,
    }
}
#[test]
fn held_motion_never_reactivates_and_release_permits_a_fresh_press() {
    let mut state = PointerState::new();
    assert!(!state.observe(sample(1, false)).unwrap());
    assert!(state.observe(sample(2, true)).unwrap());
    // The owner may accept and publish any number of fresh Shows here. It
    // does not own this physical history or get to reset a held gesture.
    for sequence in 3..20 {
        let mut motion = sample(sequence, true);
        motion.position_x += sequence as i64;
        assert!(!state.observe(motion).unwrap());
    }
    assert!(!state.observe(sample(20, false)).unwrap());
    assert!(state.observe(sample(21, true)).unwrap());
}
#[test]
fn stale_or_gapped_physical_history_cannot_authorize_a_press() {
    let mut state = PointerState::new();
    assert!(!state.observe(sample(1, false)).unwrap());
    assert_eq!(
        state.observe(sample(1, true)),
        Err("product-pointer-sample-invalid")
    );
    let mut gap = sample(2, true);
    gap.coalesced = 1;
    assert_eq!(
        state.observe(gap),
        Err("product-pointer-gesture-history-gap")
    );
    gap.coalesced = 0;
    gap.dropped = 1;
    assert_eq!(
        state.observe(gap),
        Err("product-pointer-gesture-history-gap")
    );
    assert_eq!(state.sequence, 1);
    assert!(!state.primary_pressed);
}

#[test]
fn retired_pointer_cannot_be_reactivated_by_a_fresh_sample() {
    let mut state = PointerState::new();
    state.retire();
    assert!(state.is_retired());
    assert_eq!(
        state.observe(sample(1, true)),
        Err("product-pointer-retired")
    );
}

#[test]
fn shared_pointer_position_round_trips_display_pixels_and_clamps_normalized_edges() {
    use crate::display::{DisplayError, DisplayFormat, PixelTarget};
    struct Display;
    impl PixelTarget for Display {
        fn format(&self) -> DisplayFormat {
            DisplayFormat {
                width: 641,
                height: 479,
                pitch: 641 * 4,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            }
        }
        fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
            panic!("position observation must not render")
        }
    }
    let mut state = PointerState::new();
    for (index, (x, y)) in [(0_u16, 0_u16), (320, 239), (640, 478)]
        .into_iter()
        .enumerate()
    {
        let mut point = sample(index as u64 + 1, false);
        point.position_x = crate::pointer_coordinates::normalized_local(x, 641).unwrap();
        point.position_y = crate::pointer_coordinates::normalized_local(y, 479).unwrap();
        state.observe(point).unwrap();
        assert_eq!(
            state.position(&Display).unwrap(),
            (u32::from(x), u32::from(y))
        );
    }
    let mut edge = sample(4, false);
    edge.position_x = 1_000_000;
    edge.position_y = 1_000_000;
    state.observe(edge).unwrap();
    assert_eq!(state.position(&Display).unwrap(), (640, 478));
}

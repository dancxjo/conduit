use super::*;
#[test]
fn selected_slice320x40_accepts_last_window_and_refuses_overrun_or_pressure() {
    let input = core::array::from_fn(|i| i as f32);
    exercise::<320, 40, 4, 3>(
        FixedIndexOperation::Slice,
        input,
        &[280],
        core::array::from_fn(|i| (280 + i) as f32),
        false,
        false,
    );
    exercise::<320, 40, 4, 3>(
        FixedIndexOperation::Slice,
        input,
        &[281],
        [0.; 40],
        true,
        false,
    );
    exercise::<320, 40, 4, 3>(
        FixedIndexOperation::Slice,
        input,
        &[0],
        core::array::from_fn(|i| i as f32),
        false,
        true,
    );
}

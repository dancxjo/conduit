//! Canonical boundary fallback of reviewed compact owners, including refusal.
use super::*;
#[test]
fn reviewed_slices_preserve_boundary_values_bounds_and_pressure_before_commit() {
    for (start, overflow, pressure) in [
        (0, false, false),
        (320, false, false),
        (321, true, false),
        (160, false, true),
    ] {
        let input = std::array::from_fn(|i| (i as f32 - 192.) * 0.03125);
        let expected = std::array::from_fn(|i| if overflow { 0. } else { input[start + i] });
        exercise_owner::<480, 160, 4, 3>(
            FixedIndexOperation::Slice,
            input,
            &[start as u16],
            expected,
            overflow,
            pressure,
            true,
        );
    }
    for (start, overflow, pressure) in [
        (0, false, false),
        (256, false, false),
        (257, true, false),
        (128, false, true),
    ] {
        let input = std::array::from_fn(|i| (i as f32 - 192.) * 0.03125);
        let expected = std::array::from_fn(|i| if overflow { 0. } else { input[start + i] });
        exercise_owner::<384, 128, 4, 3>(
            FixedIndexOperation::Slice,
            input,
            &[start as u16],
            expected,
            overflow,
            pressure,
            true,
        );
    }
}

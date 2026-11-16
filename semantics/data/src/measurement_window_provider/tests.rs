extern crate std;
use super::*;
use crate::*;
use alloc::vec;
use conduit_core::{Quantity, StructuredInfoValue, TemporalInstant, TemporalScale, Unit};
fn canonical(ty: conduit_core::StructuredInfoType, bytes: Vec<u8>) -> Vec<u8> {
    StructuredInfoValue::leaf(ty, bytes)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn profile(capacity: u8, policy: FullWindowPolicy) -> Vec<u8> {
    canonical(
        measurement_window_profile_type(),
        encode_measurement_window_profile(&MeasurementWindowProfile {
            capacity,
            range: MeasurementRange {
                minimum: Quantity::new(0, Unit::Millivolt),
                maximum: Quantity::new(100, Unit::Millivolt),
            },
            clock_basis: "source/boot".into(),
            full_policy: policy,
        })
        .unwrap(),
    )
}
fn sample(ticks: u64, value: i64, clock: &str, unit: Unit) -> Vec<u8> {
    canonical(
        measurement_sample_type(),
        encode_measurement_sample(&MeasurementSample {
            value: Quantity::new(value, unit),
            observed_at: TemporalInstant {
                ticks,
                scale: TemporalScale::Milliseconds,
                clock_basis: clock.into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            }
            .try_into()
            .unwrap(),
            uncertainty: None,
        })
        .unwrap(),
    )
}
fn configured(capacity: u8, policy: FullWindowPolicy) -> PreparedMeasurementWindow {
    let mut p = PreparedMeasurementWindow::new();
    p.execute(
        RequestId(0),
        MEASUREMENT_WINDOW_PROFILE_CALL,
        &profile(capacity, policy),
    )
    .unwrap();
    p.commit(RequestId(0)).unwrap();
    p
}
#[test]
fn candidates_and_exact_retries_do_not_advance_until_actual_commit() {
    let mut p = PreparedMeasurementWindow::new();
    let profile = profile(2, FullWindowPolicy::Reject);
    p.execute(RequestId(0), MEASUREMENT_WINDOW_PROFILE_CALL, &profile)
        .unwrap();
    assert!(p.committed_window().is_none());
    p.execute(RequestId(0), MEASUREMENT_WINDOW_PROFILE_CALL, &profile)
        .unwrap();
    assert!(p
        .execute(
            RequestId(1),
            MEASUREMENT_WINDOW_SAMPLE_CALL,
            &sample(1, 10, "source/boot", Unit::Millivolt)
        )
        .is_err());
    p.commit(RequestId(0)).unwrap();
    let first = sample(1, 10, "source/boot", Unit::Millivolt);
    p.execute(RequestId(1), MEASUREMENT_WINDOW_SAMPLE_CALL, &first)
        .unwrap();
    p.execute(RequestId(1), MEASUREMENT_WINDOW_SAMPLE_CALL, &first)
        .unwrap();
    assert_eq!(p.committed_window().unwrap().samples().len(), 0);
    assert!(p.commit(RequestId(99)).is_err());
    assert!(p
        .execute(
            RequestId(1),
            MEASUREMENT_WINDOW_SAMPLE_CALL,
            &sample(2, 20, "source/boot", Unit::Millivolt)
        )
        .is_err());
    p.commit(RequestId(1)).unwrap();
    assert_eq!(p.committed_window().unwrap().samples().len(), 1);
    p.execute(RequestId(1), MEASUREMENT_WINDOW_SAMPLE_CALL, &first)
        .unwrap();
    assert_eq!(p.committed_window().unwrap().samples().len(), 1);
}
#[test]
fn empty_window_and_drop_oldest_follow_existing_semantics_without_total64_cutoff() {
    let mut empty = configured(2, FullWindowPolicy::Reject);
    std::println!(
        "Window canonical bounds={:?}; prepared buffers={}; dynamic workspace={}",
        measurement_window_value_bounds(),
        empty.prepared_backing_bytes(),
        measurement_window_dynamic_workspace_bytes()
    );
    let bytes = empty
        .execute(
            RequestId(1),
            MEASUREMENT_WINDOW_SAMPLE_CALL,
            MEASUREMENT_WINDOW_FINALIZE,
        )
        .unwrap()
        .unwrap();
    let ty = measurement_window_type().canonical_bytes().unwrap();
    let decoded =
        decode_measurement_window(payload(bytes, &ty, MAXIMUM_MEASUREMENT_WINDOW_BYTES).unwrap())
            .unwrap();
    assert!(decoded.samples().is_empty());
    let mut p = configured(2, FullWindowPolicy::DropOldest);
    for id in 1..=65 {
        p.execute(
            RequestId(id),
            MEASUREMENT_WINDOW_SAMPLE_CALL,
            &sample(id as u64, 10, "source/boot", Unit::Millivolt),
        )
        .unwrap();
        p.commit(RequestId(id)).unwrap();
    }
    assert_eq!(p.committed_window().unwrap().samples().len(), 2);
    assert_eq!(p.committed_window().unwrap().discarded_samples(), 63);
}
#[test]
fn full_unit_clock_and_order_refusals_preserve_committed_window() {
    for (next, detail) in [
        (sample(2, 20, "source/boot", Unit::Millivolt), 20),
        (sample(2, 20, "source/boot", Unit::Hertz), 14),
        (sample(2, 20, "other/boot", Unit::Millivolt), 17),
        (sample(1, 20, "source/boot", Unit::Millivolt), 18),
        (sample(2, 101, "source/boot", Unit::Millivolt), 19),
    ] {
        let mut p = configured(1, FullWindowPolicy::Reject);
        p.execute(
            RequestId(1),
            MEASUREMENT_WINDOW_SAMPLE_CALL,
            &sample(1, 10, "source/boot", Unit::Millivolt),
        )
        .unwrap();
        p.commit(RequestId(1)).unwrap();
        assert_eq!(
            p.execute(RequestId(2), MEASUREMENT_WINDOW_SAMPLE_CALL, &next)
                .unwrap_err()
                .detail,
            detail
        );
        assert_eq!(
            p.committed_window().unwrap().samples()[0].value,
            Quantity::new(10, Unit::Millivolt)
        );
        assert!(p.commit(RequestId(2)).is_err());
    }
}
#[test]
fn mismatched_type_and_oversized_input_never_initialize_state() {
    let mut p = PreparedMeasurementWindow::new();
    let b = measurement_window_value_bounds();
    assert!(p
        .execute(
            RequestId(0),
            MEASUREMENT_WINDOW_PROFILE_CALL,
            &vec![0; b[0] as usize + 1]
        )
        .is_err());
    assert!(p.committed_window().is_none());
    assert!(p
        .execute(
            RequestId(0),
            MEASUREMENT_WINDOW_PROFILE_CALL,
            &sample(1, 10, "source/boot", Unit::Millivolt)
        )
        .is_err());
    assert!(p.commit(RequestId(0)).is_err());
}

#[test]
fn cancelled_provider_cannot_replay_or_commit_a_late_prepared_result() {
    let mut p = PreparedMeasurementWindow::new();
    let bytes = profile(2, FullWindowPolicy::Reject);
    p.execute(RequestId(0), MEASUREMENT_WINDOW_PROFILE_CALL, &bytes)
        .unwrap();
    p.cancel();
    assert!(p
        .execute(RequestId(0), MEASUREMENT_WINDOW_PROFILE_CALL, &bytes)
        .is_err());
    assert!(p.commit(RequestId(0)).is_err());
    assert!(p.committed_window().is_none());
}

#[test]
fn actual_authored_clock_text_bound_refuses_without_initializing_window() {
    let bytes = canonical(
        measurement_window_profile_type(),
        encode_measurement_window_profile(&MeasurementWindowProfile {
            capacity: 2,
            range: MeasurementRange {
                minimum: Quantity::new(0, Unit::Millivolt),
                maximum: Quantity::new(100, Unit::Millivolt),
            },
            clock_basis: "x".repeat(129),
            full_policy: FullWindowPolicy::Reject,
        })
        .unwrap(),
    );
    let mut p = PreparedMeasurementWindow::new();
    assert_eq!(
        p.execute(RequestId(0), MEASUREMENT_WINDOW_PROFILE_CALL, &bytes)
            .unwrap_err()
            .detail,
        11
    );
    assert!(p.committed_window().is_none());
    assert!(p.commit(RequestId(0)).is_err());
}

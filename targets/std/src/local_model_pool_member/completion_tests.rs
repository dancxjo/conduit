use super::*;

#[test]
fn prepared_sampling_and_request_hashing_do_not_allocate() {
    let mut measured = slot();
    let allocations = crate::allocation_probe::begin();
    measured.start(b"request");
    measured.finish(LocalModelAdapterTerminal::Produced, 5);
    assert_eq!(allocations.finish(), 0);
    assert!(measured.prepare_times().is_ok());
}

fn slot() -> MeasurementSlot {
    let clock = MonotonicClockIdentity::new(
        "measured-host".into(),
        "measured-boot".into(),
        "fixture/provider-epoch".into(),
        conduit_core::TemporalScale::Microseconds,
        2,
        3,
    )
    .unwrap();
    MeasurementSlot {
        clock: Some(clock),
        bounds: Some((8, 8)),
        started: None,
        completed: None,
        input_bytes: 0,
        output_bytes: 0,
        input_digest: [0; 32],
        terminal: None,
        refusal: None,
    }
}

#[test]
fn completed_interval_retains_clock_quality_and_exact_request_custody() {
    let mut measured = slot();
    assert_eq!(
        measured.prepare_times().unwrap_err(),
        LocalModelMeasurementRefusal::NotCompleted
    );
    measured.start(b"request");
    measured.finish(LocalModelAdapterTerminal::Produced, 5);
    // A deterministic clock interval fixture; production uses ThreadTimer.
    measured.started = Some(12);
    measured.completed = Some(27);
    let (started, completed, elapsed, terminal) = measured.prepare_times().unwrap();
    assert_eq!(started.clock(), completed.clock());
    assert_eq!(started.clock().host_id().as_str(), "measured-host");
    assert_eq!(started.clock().boot_id().as_str(), "measured-boot");
    assert_eq!(started.clock().resolution_ticks(), 2);
    assert_eq!(started.clock().uncertainty_ticks(), 3);
    assert_eq!(elapsed.ticks(), 15);
    assert_eq!(elapsed.scale(), conduit_core::TemporalScale::Microseconds);
    assert_eq!(terminal, LocalModelAdapterTerminal::Produced);
    assert_eq!(measured.input_bytes, 7);
    assert_eq!(measured.output_bytes, 5);
    assert_eq!(
        measured.input_digest,
        conduit_core::semantic_digest("conduit.std/local-model-completed-input@1", b"request")
    );
    assert_ne!(
        measured.input_digest,
        conduit_core::semantic_digest("conduit.std/local-model-completed-input@1", b"changed")
    );
}

#[test]
fn failed_terminals_remain_measured_outcomes_instead_of_successful_work() {
    for terminal in [
        LocalModelAdapterTerminal::ProviderLost,
        LocalModelAdapterTerminal::Cancelled,
        LocalModelAdapterTerminal::Refused,
        LocalModelAdapterTerminal::InvalidStructuredResult,
        LocalModelAdapterTerminal::Truncated,
    ] {
        let mut measured = slot();
        measured.start(b"request");
        measured.finish(terminal, 0);
        assert_eq!(measured.prepare_times().unwrap().3, terminal);
    }
}

#[test]
fn unavailable_clock_and_regression_never_become_known_zero_duration() {
    let mut measured = slot();
    measured.clock = None;
    measured.start(b"request");
    measured.finish(LocalModelAdapterTerminal::Produced, 1);
    assert_eq!(
        measured.prepare_times().unwrap_err(),
        LocalModelMeasurementRefusal::ClockUnavailable
    );
    measured = slot();
    measured.start(b"request");
    measured.finish(LocalModelAdapterTerminal::Produced, 1);
    measured.started = Some(27);
    measured.completed = Some(12);
    assert!(matches!(
        measured.prepare_times(),
        Err(LocalModelMeasurementRefusal::Clock(_))
    ));
}

#[test]
fn declared_input_and_output_bounds_refuse_without_truncating_custody() {
    for (input, output, expected) in [
        (
            b"oversized".as_slice(),
            0,
            LocalModelMeasurementRefusal::InputExtent,
        ),
        (
            b"request".as_slice(),
            9,
            LocalModelMeasurementRefusal::OutputExtent,
        ),
    ] {
        let mut measured = slot();
        measured.start(input);
        measured.finish(LocalModelAdapterTerminal::Produced, output);
        assert_eq!(measured.prepare_times().unwrap_err(), expected);
    }
    let mut measured = slot();
    measured.bounds = None;
    measured.start(b"request");
    measured.finish(LocalModelAdapterTerminal::Produced, 0);
    assert_eq!(
        measured.prepare_times().unwrap_err(),
        LocalModelMeasurementRefusal::MissingBounds
    );
}

#[test]
#[ignore = "manual bounded measurement-processing characterization, not provider performance"]
fn characterize_measurement_processing() {
    use std::{hint::black_box, time::Instant};
    const CYCLES: u32 = 1000;
    for bytes in [1024, 16 * 1024, 64 * 1024] {
        let input = vec![b'x'; bytes];
        let expected =
            conduit_core::semantic_digest("conduit.std/local-model-completed-input@1", &input);
        for trial in 0..8 {
            let mut measured = slot();
            measured.bounds = Some((bytes as u32, 8));
            // Prepare inputs/storage/counter initialization outside timing.
            measured.start(&input);
            measured.finish(LocalModelAdapterTerminal::Produced, 5);
            let started = Instant::now();
            for _ in 0..CYCLES {
                measured.start(black_box(&input));
                measured.finish(LocalModelAdapterTerminal::Produced, 5);
                black_box(measured.input_digest);
            }
            let elapsed = started.elapsed().as_nanos();
            assert_eq!(measured.input_digest, expected);
            assert_eq!(measured.input_bytes, bytes as u32);
            println!("MEASUREMENT_PROCESSING bytes={bytes} trial={trial} cycles={CYCLES} elapsed_ns={elapsed}");
        }
    }
}

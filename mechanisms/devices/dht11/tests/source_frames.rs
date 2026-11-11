//! Physical capture fixtures pass through checked Source and authored Forms.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{validate_canonical_structured_value as validate, Quantity, Unit};
use conduit_dht11_observation::{ObservationContract, PreparedSourceFrameDecoder};
#[test]
fn captured_frames_become_quantities_only_after_source_validation_without_allocating() {
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let expected_t = Quantity::from_decimal(202, -1, Unit::Celsius).unwrap();
    let expected_h = Quantity::new(53, Unit::Percent);
    let ((), measured) = allocation_probe::observe(|| {
        let output = decoder.decode(5, [106, 0, 40, 4, 151]).unwrap();
        let result = validate(output).unwrap();
        assert!(result.variant_payload("observation").unwrap().is_none());
        assert!(result
            .variant_payload("unavailable")
            .unwrap()
            .unwrap()
            .variant_payload("checksum")
            .unwrap()
            .is_some());
        for _ in 0..2 {
            let output = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
            let result = validate(output).unwrap();
            let frame = result.variant_payload("observation").unwrap().unwrap();
            let quantity = |name| {
                Quantity::decode(&frame.record_field(name).unwrap().unwrap().value_node()[5..])
                    .unwrap()
            };
            assert!(quantity("temperature").compare(expected_t).unwrap().is_eq());
            assert!(quantity("humidity").compare(expected_h).unwrap().is_eq());
        }
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn source_keeps_length_range_negative_and_recovery_semantics() {
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    for (length, wire, reason) in [
        (2, [0; 5], "short-frame"),
        (6, [0; 5], "malformed-length"),
        (5, [0; 5], "invalid-payload"),
    ] {
        let output = decoder.decode(length, wire).unwrap();
        assert!(validate(output)
            .unwrap()
            .variant_payload("unavailable")
            .unwrap()
            .unwrap()
            .variant_payload(reason)
            .unwrap()
            .is_some());
    }
    let output = decoder.decode(5, [53, 0, 10, 130, 193]).unwrap();
    let frame = validate(output)
        .unwrap()
        .variant_payload("observation")
        .unwrap()
        .unwrap();
    let temperature = Quantity::decode(
        &frame
            .record_field("temperature")
            .unwrap()
            .unwrap()
            .value_node()[5..],
    )
    .unwrap();
    assert!(temperature
        .compare(Quantity::from_decimal(-108, -1, Unit::Celsius).unwrap())
        .unwrap()
        .is_eq());
}

#[cfg(feature = "source-capture")]
#[test]
fn authored_capture_geometry_is_checked_against_the_selected_native_envelope() {
    let limits = conduit_pulse_capture::PulseReadLimits {
        maximum_hold_low_us: 50_000,
        maximum_skipped_pulses: 4,
        minimum_sample_offset_us: 10,
        maximum_sample_offset_us: 200,
        maximum_octets: 8,
        maximum_capture_timeout_us: 10_000,
    };
    let request = PreparedSourceFrameDecoder::capture_request(limits).unwrap();
    assert_eq!(request.request().octets, 5);
    assert_eq!(request.deadline_span_us(), 30_000);
    assert!(
        PreparedSourceFrameDecoder::capture_request(conduit_pulse_capture::PulseReadLimits {
            maximum_capture_timeout_us: 500,
            ..limits
        })
        .is_err()
    );
}

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{Quantity, Unit};
use conduit_environment_measurement::{source_observation::*, EnvironmentalReading, ReadingField};
#[test]
fn bme_precision_and_reported_clock_round_trip_without_dht_forms() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let observation = SourceObservation {
        observed_at_host_boot_ms: 9000,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::centi_celsius_centi_pascal(
                2422,
                10_134_081,
                Some(46535),
            )
            .unwrap(),
            source_counter: 28,
            reported_source_ms: Some(825388),
        }),
    };
    let bytes = codec.encode(observation).unwrap();
    assert!(bytes.len() <= MAXIMUM_BYTES);
    assert_eq!(contract.decode(bytes).unwrap(), observation);
    let SourceOutcome::Available(sample) = contract.decode(bytes).unwrap().outcome else {
        panic!()
    };
    assert_eq!(
        sample.reading.atmospheric_pressure(),
        ReadingField::Available(Quantity::from_decimal(10_134_081, -2, Unit::Pascal).unwrap())
    );
}
#[test]
fn dht_unsupported_pressure_and_unknown_acquisition_time_stay_absent() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = SourceObservation {
        observed_at_host_boot_ms: 500,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::temperature_humidity(
                Quantity::from_decimal(247, -1, Unit::Celsius).unwrap(),
                Quantity::new(36, Unit::Percent),
            )
            .unwrap(),
            source_counter: 127,
            reported_source_ms: None,
        }),
    };
    assert_eq!(
        contract.decode(codec.encode(value).unwrap()).unwrap(),
        value
    );
}
#[test]
fn failures_are_not_unsupported_success_and_zero_counter_refuses() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    for failure in [
        SourceFailure::CaptureTimeout,
        SourceFailure::ProviderFailed,
        SourceFailure::InvalidSample,
    ] {
        let value = SourceObservation {
            observed_at_host_boot_ms: 700,
            outcome: SourceOutcome::Unavailable(failure),
        };
        assert_eq!(
            contract.decode(codec.encode(value).unwrap()).unwrap(),
            value
        );
    }
    let value = SourceObservation {
        observed_at_host_boot_ms: 500,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::temperature_humidity(
                Quantity::new(24, Unit::Celsius),
                Quantity::new(36, Unit::Percent),
            )
            .unwrap(),
            source_counter: 0,
            reported_source_ms: None,
        }),
    };
    assert!(codec.encode(value).is_err());
    assert!(contract.decode(&[0; MAXIMUM_BYTES + 1]).is_err());
}

#[test]
fn compact_form_has_no_per_value_unit_metadata_and_rejects_hidden_coordinates() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = SourceObservation {
        observed_at_host_boot_ms: 500,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::centi_celsius_centi_pascal(
                2422,
                10_134_081,
                Some(46535),
            )
            .unwrap(),
            source_counter: 1,
            reported_source_ms: None,
        }),
    };
    let encoded = codec.encode(value).unwrap();
    println!(
        "environmental canonical bytes={} dynamic Form bytes={}",
        encoded.len(),
        FORM_BYTES
    );
    assert!(encoded.len() < 160);
    assert_eq!(FORM_BYTES, 82);
    let mut form = encode_form(value).unwrap();
    form[18] = 0;
    form[19] = 1;
    assert!(decode_form(&form).is_err());
    let failure = SourceObservation {
        observed_at_host_boot_ms: 900,
        outcome: SourceOutcome::Unavailable(SourceFailure::ProviderFailed),
    };
    let mut form = encode_form(failure).unwrap();
    form[28] = 1;
    assert!(decode_form(&form).is_err());
}

#[test]
fn prepared_compact_source_encoding_and_decoding_allocate_nothing() {
    let contract = ObservationContract::prepare().unwrap();
    let mut codec = PreparedObservationCodec::prepare(&contract).unwrap();
    let value = SourceObservation {
        observed_at_host_boot_ms: 500,
        outcome: SourceOutcome::Available(SourceSample {
            reading: EnvironmentalReading::centi_celsius_centi_pascal(
                2422,
                10_134_081,
                Some(46535),
            )
            .unwrap(),
            source_counter: 1,
            reported_source_ms: None,
        }),
    };
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..16 {
            assert_eq!(
                contract.decode(codec.encode(value).unwrap()).unwrap(),
                value
            );
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn physical_session_baselines_cached_samples_and_cannot_recover_from_rollback_by_polling() {
    let reading = EnvironmentalReading::temperature_humidity(
        Quantity::new(24, Unit::Celsius),
        Quantity::new(36, Unit::Percent),
    )
    .unwrap();
    let mut sample = SourceSample {
        reading,
        source_counter: 10,
        reported_source_ms: Some(300000),
    };
    let mut continuity = SourceContinuity::default();
    assert_eq!(
        continuity.observe(&sample, 100),
        Err(SourceContinuityRefusal::NoNewObservation)
    );
    assert_eq!(
        continuity.observe(&sample, 200),
        Err(SourceContinuityRefusal::NoNewObservation)
    );
    sample.source_counter = 11;
    sample.reported_source_ms = Some(330000);
    assert_eq!(continuity.observe(&sample, 301), Ok(()));
    assert_eq!(
        continuity.observe(&sample, 300),
        Err(SourceContinuityRefusal::HostClockReversed)
    );
    sample.source_counter = 12;
    sample.reported_source_ms = Some(50);
    assert_eq!(
        continuity.observe(&sample, 400),
        Err(SourceContinuityRefusal::SessionLost)
    );
    sample.source_counter = 13;
    sample.reported_source_ms = Some(360000);
    assert_eq!(
        continuity.observe(&sample, 500),
        Err(SourceContinuityRefusal::SessionLost)
    );
    let mut unknown_clock = SourceContinuity::default();
    sample.reported_source_ms = None;
    assert_eq!(
        unknown_clock.observe(&sample, 500),
        Err(SourceContinuityRefusal::NoNewObservation)
    );
    sample.source_counter = 1;
    assert_eq!(
        unknown_clock.observe(&sample, 600),
        Err(SourceContinuityRefusal::SessionLost)
    );
}

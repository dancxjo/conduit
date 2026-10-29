use super::*;
#[test]
fn actual_bosch_shape_keeps_pressure_and_q10_humidity_exact() {
    let bytes=br#"{"body_member":false,"samples":28,"attempts":28,"sensor_error":false,"temperature_centi_celsius":2422,"pressure_centi_pascal":10134081,"humidity_percent_q10":46535,"sampled_at_boot_ms":825388}"#;
    let SourceOutcome::Available(sample) = decode(bytes, Encoding::Bosch).unwrap() else {
        panic!()
    };
    assert_eq!(sample.source_counter, 28);
    assert_eq!(sample.reported_source_ms, Some(825388));
    assert_eq!(
        sample.reading.atmospheric_pressure(),
        ReadingField::Available(Quantity::from_decimal(10134081, -2, Unit::Pascal).unwrap())
    );
    assert_eq!(
        sample.reading.relative_humidity(),
        ReadingField::Available(
            Quantity::from_decimal(46535 * 9_765_625i128, -10, Unit::Percent).unwrap()
        )
    );
}
#[test]
fn actual_dht_shape_keeps_pressure_and_acquisition_clock_unsupported() {
    let bytes=br#"{"body_member":false,"samples":127,"attempts":128,"status":"ok","temperature":{"coefficient":247,"exponent":-1},"humidity":{"coefficient":36,"exponent":0}}"#;
    let SourceOutcome::Available(sample) = decode(bytes, Encoding::Dht11).unwrap() else {
        panic!()
    };
    assert_eq!(sample.reported_source_ms, None);
    assert_eq!(
        sample.reading.atmospheric_pressure(),
        ReadingField::Unsupported
    );
    assert_eq!(
        sample.reading.temperature(),
        Quantity::from_decimal(247, -1, Unit::Celsius).unwrap()
    );
}
#[test]
fn duplicates_counter_errors_and_bad_success_shapes_do_not_become_readings() {
    for bytes in [
        &br#"{"body_member":false,"samples":2,"samples":3,"attempts":3,"status":"ok"}"#[..],
        br#"{"body_member":true,"samples":0,"attempts":1,"status":"warming"}"#,
        br#"{"body_member":false,"samples":2,"attempts":1,"status":"warming"}"#,
        br#"{"body_member":false,"samples":2,"attempts":3,"status":"ok"}"#,
    ] {
        assert!(decode(bytes, Encoding::Dht11).is_err());
    }
    let failed = br#"{"body_member":false,"samples":1,"attempts":2,"sensor_error":true}"#;
    assert!(matches!(
        decode(failed, Encoding::Bosch).unwrap(),
        SourceOutcome::Unavailable(SourceFailure::ProviderFailed)
    ));
}

#[test]
fn unsupported_bosch_humidity_is_explicit_null_not_a_missing_success_field() {
    let bmp=br#"{"body_member":false,"samples":1,"attempts":1,"sensor_error":false,"temperature_centi_celsius":2422,"pressure_centi_pascal":10134081,"humidity_percent_q10":null}"#;
    let SourceOutcome::Available(sample) = decode(bmp, Encoding::Bosch).unwrap() else {
        panic!()
    };
    assert_eq!(
        sample.reading.relative_humidity(),
        ReadingField::Unsupported
    );
    let missing=br#"{"body_member":false,"samples":1,"attempts":1,"sensor_error":false,"temperature_centi_celsius":2422,"pressure_centi_pascal":10134081}"#;
    assert!(decode(missing, Encoding::Bosch).is_err());
}

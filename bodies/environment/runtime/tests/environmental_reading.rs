use conduit_core::{Quantity, QuantityRole, Unit};
use conduit_environment_runtime::environmental_reading::{
    EnvironmentalReading as Reading, ReadingField as Field, ReadingRefusal as R,
};

#[test]
fn legacy_pair_preserves_exact_quantities_and_encoded_bytes() {
    for (t, h) in [(-399, 0), (0, 500), (801, 1000)] {
        let temperature = Quantity::from_decimal(t, -1, Unit::Celsius).unwrap();
        let humidity = Quantity::from_decimal(h, -1, Unit::Percent).unwrap();
        let reading = Reading::temperature_humidity(temperature, humidity).unwrap();
        assert_eq!(reading.temperature().encode(), temperature.encode());
        assert_eq!(reading.relative_humidity(), Field::Available(humidity));
        assert_eq!(reading.atmospheric_pressure(), Field::Unsupported);
    }
}

#[test]
fn bosch_fixed_coordinates_are_exact_and_bmp_has_no_humidity() {
    let pressure = Quantity::from_decimal(10_065_328, -2, Unit::Pascal).unwrap();
    for temperature in [-4000, 0, 2508, 8500] {
        for humidity in 0..=102_400 {
            let reading =
                Reading::centi_celsius_centi_pascal(temperature, 10_065_328, Some(humidity))
                    .unwrap();
            let expected =
                Quantity::from_decimal(i128::from(humidity) * 9_765_625, -10, Unit::Percent)
                    .unwrap();
            assert_eq!(reading.relative_humidity(), Field::Available(expected));
            assert_eq!(reading.atmospheric_pressure(), Field::Available(pressure));
            assert_eq!(
                reading.temperature(),
                Quantity::from_decimal(i128::from(temperature), -2, Unit::Celsius).unwrap()
            );
        }
    }
    let bmp = Reading::centi_celsius_centi_pascal(2508, 10_065_328, None).unwrap();
    assert_eq!(bmp.relative_humidity(), Field::Unsupported);
    assert_eq!(bmp.atmospheric_pressure(), Field::Available(pressure));
    assert_eq!(
        Reading::centi_celsius_centi_pascal(0, 0, None),
        Err(R::AtmosphericPressure)
    );
    assert_eq!(
        Reading::centi_celsius_centi_pascal(0, 1, Some(102_401)),
        Err(R::RelativeHumidity)
    );
}

#[test]
fn wrong_families_roles_and_ranges_refuse() {
    let t = Quantity::new(20, Unit::Celsius);
    let none = Field::Unsupported;
    assert_eq!(
        Reading::new(Quantity::new(1, Unit::Pascal), none, none),
        Err(R::Temperature)
    );
    assert_eq!(
        Reading::new(t, Field::Available(t), none),
        Err(R::RelativeHumidity)
    );
    for h in [-1, 101] {
        assert_eq!(
            Reading::new(t, Field::Available(Quantity::new(h, Unit::Percent)), none),
            Err(R::RelativeHumidity)
        );
    }
    assert_eq!(
        Reading::new(t, none, Field::Available(Quantity::new(1, Unit::Percent))),
        Err(R::AtmosphericPressure)
    );
    assert_eq!(
        Reading::new(t, none, Field::Available(Quantity::new(-1, Unit::Pascal))),
        Err(R::AtmosphericPressure)
    );
}

#[test]
fn affine_temperature_delta_is_not_an_environmental_temperature_point() {
    let delta = Quantity::from_decimal_role(20, 0, Unit::Celsius, QuantityRole::Delta).unwrap();
    assert_eq!(
        Reading::new(delta, Field::Unsupported, Field::Unsupported),
        Err(R::Temperature)
    );
}

#[test]
fn pressure_look_conversion_does_not_change_measurement() {
    let reading = Reading::centi_celsius_centi_pascal(2508, 10_065_328, None).unwrap();
    let Field::Available(pressure) = reading.atmospheric_pressure() else {
        panic!("pressure missing")
    };
    let hpa = Unit::resolve("hPa").unwrap();
    let shown = pressure.convert(hpa).unwrap();
    assert_eq!(shown, Quantity::from_decimal(10_065_328, -4, hpa).unwrap());
    assert_eq!(shown.convert(Unit::Pascal).unwrap(), pressure);
    assert_eq!(reading.atmospheric_pressure(), Field::Available(pressure));
}

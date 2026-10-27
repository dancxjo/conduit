#[path = "../src/environment_show.rs"]
mod show;
use conduit_bosch_env::{Humidity, HumidityUnavailable, Reading};
fn reading(humidity: Humidity) -> Reading {
    Reading {
        temperature_centi_celsius: 2500,
        pressure_centi_pascal: 10132500,
        humidity,
    }
}
#[test]
fn actual_bosch_coordinates_use_common_face_and_whole_degree_look() {
    let bme = reading(Humidity::PercentQ10(44 * 1024));
    let temp = show::render(bme, 1, 0).unwrap();
    let rh = show::render(bme, 1, 2000).unwrap();
    assert_ne!(temp, rh);
    assert_eq!(temp[1] & 0x80, 0, "whole temperature has no colon");
    assert_ne!(rh[1] & 0x80, 0, "shared rH:44 Look owns colon");
    let bmp = reading(Humidity::Unavailable(HumidityUnavailable::UnsupportedChip));
    assert_eq!(show::render(bmp, 2, 0).unwrap(), temp);
    assert_eq!(
        show::render(bmp, 2, 2000).unwrap(),
        [0x40, 0x40, 0x40, 0x40, 0, 0]
    );
    let mut pressure = bme;
    pressure.pressure_centi_pascal = 9000000;
    assert_eq!(
        show::render(pressure, 3, 0).unwrap(),
        temp,
        "pressure is a Face quantity, not a fabricated display page"
    );
    assert_eq!(
        show::render(bme, 1, 30000).unwrap(),
        [0x40, 0x40, 0x40, 0x40, 0, 0]
    );
}
#[test]
fn invalid_reading_coordinates_are_refused_without_fabricated_display() {
    let mut invalid = reading(Humidity::PercentQ10(102401));
    assert!(show::render(invalid, 1, 0).is_err());
    invalid.humidity = Humidity::PercentQ10(44 * 1024);
    invalid.pressure_centi_pascal = 0;
    assert!(show::render(invalid, 1, 0).is_err());
}

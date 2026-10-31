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
    assert_eq!(rh, [0x76, 0, 0x66, 0x66, 0, 0], "H 44, no colon");
    let pressure_page = [0x06, 0x3f, 0x06, 0x4f, 0, 0]; // 1013 hPa
    assert_eq!(show::render(bme, 1, 4000).unwrap(), pressure_page);
    assert_eq!(show::render(bme, 1, 6000).unwrap(), temp);
    assert_eq!(
        show::render(reading(Humidity::PercentQ10(100 * 1024)), 1, 2000).unwrap(),
        [0x76, 0x06, 0x3f, 0x3f, 0, 0]
    );
    let bmp = reading(Humidity::Unavailable(HumidityUnavailable::UnsupportedChip));
    assert_eq!(show::render(bmp, 2, 0).unwrap(), temp);
    assert_eq!(
        show::render(bmp, 2, 2000).unwrap(),
        pressure_page,
        "BMP skips unsupported humidity and shows actual pressure"
    );
    let mut pressure = bme;
    pressure.pressure_centi_pascal = 9000000;
    assert_eq!(
        show::render(pressure, 3, 0).unwrap(),
        temp,
        "pressure does not change the temperature page"
    );
    assert_ne!(show::render(pressure, 3, 4000).unwrap(), pressure_page);
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

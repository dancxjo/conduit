use super::*;
const BODY:&[u8]=br#"{"latitude":47.6,"longitude":-122.3,"generationtime_ms":0.03,"utc_offset_seconds":0,"timezone":"GMT","timezone_abbreviation":"GMT","elevation":50.0,"current_units":{"time":"unixtime","interval":"seconds","temperature_2m":"\u00b0C","weather_code":"wmo code"},"current":{"time":100,"interval":900,"temperature_2m":-2.125,"weather_code":63}}"#;

#[test]
fn mutable_wire_matches_independent_native_projection_without_growth() {
    let native = PreparedOpenMeteoWeather::prepare("Seattle", 5_000).unwrap();
    let mut wire = PreparedCurrentWeatherWire::prepare("Seattle", 5_000).unwrap();
    let unavailable = native
        .unavailable("Weather provider unavailable")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let backing = wire.prepared_heap_bytes();
    let probe = crate::allocation_probe::begin();
    assert_eq!(wire.encode(0).unwrap(), unavailable);
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    for (time, temp, code) in [
        (100, "-2.125", 63),
        (200, "1.234e1", 0),
        (300, "100", 99),
        (400, "-100", 97),
    ] {
        let body = String::from_utf8(BODY.to_vec())
            .unwrap()
            .replace("\"time\":100", &format!("\"time\":{time}"))
            .replace("-2.125", temp)
            .replace("\"weather_code\":63", &format!("\"weather_code\":{code}"));
        let checked = native.decode_reading(body.as_bytes()).unwrap();
        let probe = crate::allocation_probe::begin();
        wire.refresh(body.as_bytes(), time * 1000).unwrap();
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
        for now in [
            time * 1000 - 1,
            time * 1000,
            time * 1000 + 5000,
            time * 1000 + 5001,
            u64::MAX,
        ] {
            let expected = if now < time * 1000 {
                native
                    .unavailable("Clock precedes weather validity")
                    .unwrap()
            } else {
                native.classify(&checked, now).unwrap()
            }
            .canonical_bytes()
            .unwrap();
            let probe = crate::allocation_probe::begin();
            assert_eq!(wire.encode(now).unwrap(), expected);
            assert_eq!(probe.finish_statistics().requested_bytes, 0);
            assert_eq!(wire.prepared_heap_bytes(), backing);
        }
    }
}

#[test]
fn rejected_refreshes_preserve_last_reading_without_error_allocations() {
    let mut wire = PreparedCurrentWeatherWire::prepare("Seattle", 5_000).unwrap();
    let native = PreparedOpenMeteoWeather::prepare("Seattle", 5_000).unwrap();
    let reading = native.decode_reading(BODY).unwrap();
    wire.refresh(BODY, 100_000).unwrap();
    let text = String::from_utf8(BODY.to_vec()).unwrap();
    let cases = [
        (
            text.replace("-2.125", "0.0001"),
            AmbientWeatherRefusal::TemperaturePrecision,
        ),
        (
            text.replace("-2.125", "100.001"),
            AmbientWeatherRefusal::NativeContract,
        ),
        (
            text.replace("\"weather_code\":63", "\"weather_code\":4"),
            AmbientWeatherRefusal::UnknownWeatherCode(4),
        ),
        (
            text.replace("unixtime", "iso8601"),
            AmbientWeatherRefusal::UnsupportedUnits,
        ),
        (
            text.replace("\"time\":100", "\"time\":101"),
            AmbientWeatherRefusal::FutureValidTime,
        ),
        (
            text.replace("\"time\":100", "\"time\":18446744073709551615"),
            AmbientWeatherRefusal::TimestampOverflow,
        ),
        (
            text.replace("\"time\":100", "\"time\":18446744073709551616"),
            AmbientWeatherRefusal::TimestampOverflow,
        ),
        (
            text.replace("\"time\":100", "\"time\":100,\"t\\u0069me\":100"),
            AmbientWeatherRefusal::MalformedResponse,
        ),
        (
            format!("{text}null"),
            AmbientWeatherRefusal::MalformedResponse,
        ),
        (
            text.replace("-2.125", "01"),
            AmbientWeatherRefusal::MalformedResponse,
        ),
        (
            text.replace("-2.125", "null"),
            AmbientWeatherRefusal::MalformedResponse,
        ),
        ("{".into(), AmbientWeatherRefusal::MalformedResponse),
        (" ".repeat(4097), AmbientWeatherRefusal::ResponseTooLarge),
    ];
    for (body, error) in cases {
        let expected = native
            .classify(&reading, 105_001)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let probe = crate::allocation_probe::begin();
        assert_eq!(wire.refresh(body.as_bytes(), 100_000), Err(error));
        assert_eq!(wire.encode(105_001).unwrap(), expected);
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
    }
}

#[test]
fn validates_discarded_json_and_bounded_nesting() {
    let text = String::from_utf8(BODY.to_vec()).unwrap();
    for ignored in [
        "[true,false,null,{\"escaped\":\"\\ud83c\\udf26\"}]",
        "{\"x\":-1.2e+3}",
    ] {
        let body = format!("{{\"ignored\":{ignored},{}", &text[1..]);
        assert!(decode(body.as_bytes()).is_ok());
    }
    for ignored in [
        "[1,]",
        "{\"x\":}",
        "\"\\ud800\"",
        "\"\\udc00\"",
        "\"\\q\"",
        "00",
        "truefalse",
    ] {
        let body = format!("{{\"ignored\":{ignored},{}", &text[1..]);
        let probe = crate::allocation_probe::begin();
        assert!(decode(body.as_bytes()).is_err());
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
    }
    let nested = format!(
        "{{\"ignored\":{}0{},{}",
        "[".repeat(17),
        "]".repeat(17),
        &text[1..]
    );
    assert!(decode(nested.as_bytes()).is_err());
}

#[test]
fn every_condition_and_precision_lexeme_matches_checked_native_values() {
    let native = PreparedOpenMeteoWeather::prepare("Seattle", 1).unwrap();
    let mut wire = PreparedCurrentWeatherWire::prepare("Seattle", 1).unwrap();
    let template = String::from_utf8(BODY.to_vec()).unwrap();
    for code in [
        0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82, 85,
        86, 95, 96, 97, 99,
    ] {
        for temperature in ["-2.1250", "7e0", "700e-2", "-0", "100.000"] {
            let body = template
                .replace("-2.125", temperature)
                .replace("\"weather_code\":63", &format!("\"weather_code\":{code}"))
                .replace("temperature_2m", "temperat\\u0075re_2m");
            let reading = native.decode_reading(body.as_bytes()).unwrap();
            let expected = native
                .classify(&reading, 100_000)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let probe = crate::allocation_probe::begin();
            wire.refresh(body.as_bytes(), 100_000).unwrap();
            assert_eq!(wire.encode(100_000).unwrap(), expected);
            assert_eq!(probe.finish_statistics().requested_bytes, 0);
        }
    }
}

#[test]
fn initial_refusal_and_exact_body_limit_remain_explicit_and_bounded() {
    let native = PreparedOpenMeteoWeather::prepare("Seattle", 5_000).unwrap();
    let mut wire = PreparedCurrentWeatherWire::prepare("Seattle", 5_000).unwrap();
    let expected = native
        .unavailable("Weather provider unavailable")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let probe = crate::allocation_probe::begin();
    assert_eq!(
        wire.refresh(b"{", 100_000),
        Err(AmbientWeatherRefusal::MalformedResponse)
    );
    assert_eq!(wire.encode(100_000).unwrap(), expected);
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    let mut body = BODY.to_vec();
    body.resize(4096, b' ');
    let probe = crate::allocation_probe::begin();
    wire.refresh(&body, 100_000).unwrap();
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    let overflow = String::from_utf8(BODY.to_vec())
        .unwrap()
        .replace("\"time\":100", "\"time\":18446744073709551");
    let probe = crate::allocation_probe::begin();
    assert_eq!(
        wire.refresh(overflow.as_bytes(), u64::MAX),
        Err(AmbientWeatherRefusal::TimestampOverflow)
    );
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
}

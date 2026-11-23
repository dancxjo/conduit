use super::*;

fn prepared() -> PreparedOpenMeteoWeather {
    PreparedOpenMeteoWeather::prepare("Seattle", 5_000).unwrap()
}

fn response() -> serde_json::Value {
    serde_json::json!({
        "timezone":"America/Los_Angeles", "utc_offset_seconds":-28800,
        "current_units":{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"},
        "current":{"time":100,"interval":900,"temperature_2m":-2.125,"weather_code":63}
    })
}

fn encode(response: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(response).unwrap()
}

fn text(value: &StructuredInfoValue, name: &str) -> String {
    rust_binding::primitive_from_structured(field(value, name).unwrap()).unwrap()
}

fn number(value: &StructuredInfoValue, name: &str) -> u64 {
    rust_binding::primitive_from_structured(field(value, name).unwrap()).unwrap()
}

fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("weather result must preserve its availability variant")
    };
    tag
}

#[test]
fn provider_validity_is_utc_and_expiry_uses_explicit_policy_not_aggregation_interval() {
    let decoder = prepared();
    let reading = decoder.decode_reading(&encode(&response())).unwrap();
    assert_eq!(text(&reading, "location"), "Seattle");
    assert_eq!(text(&reading, "attribution"), ATTRIBUTION);
    assert_eq!(text(&reading, "conditions"), "Rain, moderate");
    let temperature: i64 = rust_binding::primitive_from_structured(
        field(&reading, "temperature_celsius_milli").unwrap(),
    )
    .unwrap();
    assert_eq!(temperature, -2_125);
    assert_eq!(number(&reading, "observed_at_unix_ms"), 100_000);
    assert_eq!(number(&reading, "usable_until_unix_ms"), 105_000);
    assert_eq!(tag(&decoder.classify(&reading, 100_000).unwrap()), "fresh");
    assert_eq!(tag(&decoder.classify(&reading, 105_000).unwrap()), "fresh");
    assert_eq!(tag(&decoder.classify(&reading, 105_001).unwrap()), "stale");
    assert_eq!(
        decoder.classify(&reading, 99_999),
        Err(AmbientWeatherRefusal::FutureValidTime)
    );
}

#[test]
fn wrong_or_missing_units_never_become_valid_conditions() {
    let decoder = prepared();
    for (field, bad) in [
        ("time", "iso8601"),
        ("temperature_2m", "°F"),
        ("weather_code", ""),
    ] {
        let mut body = response();
        body["current_units"][field] = bad.into();
        assert_eq!(
            decoder.decode_reading(&encode(&body)),
            Err(AmbientWeatherRefusal::UnsupportedUnits)
        );
        body["current_units"].as_object_mut().unwrap().remove(field);
        assert_eq!(
            decoder.decode_reading(&encode(&body)),
            Err(AmbientWeatherRefusal::MalformedResponse)
        );
    }
}

#[test]
fn bounded_decoding_refuses_unknown_null_and_out_of_range_data() {
    let decoder = prepared();
    assert_eq!(
        decoder.decode_reading(&vec![b' '; MAXIMUM_OPEN_METEO_RESPONSE_BYTES + 1]),
        Err(AmbientWeatherRefusal::ResponseTooLarge)
    );
    assert_eq!(
        decoder.decode_reading(b"{"),
        Err(AmbientWeatherRefusal::MalformedResponse)
    );
    let mut body = response();
    body["current"]["weather_code"] = 4.into();
    assert_eq!(
        decoder.decode_reading(&encode(&body)),
        Err(AmbientWeatherRefusal::UnknownWeatherCode(4))
    );
    body["current"]["weather_code"] = 0.into();
    body["current"]["temperature_2m"] = serde_json::Value::Null;
    assert_eq!(
        decoder.decode_reading(&encode(&body)),
        Err(AmbientWeatherRefusal::MalformedResponse)
    );
    body["current"]["temperature_2m"] = 100.001.into();
    assert_eq!(
        decoder.decode_reading(&encode(&body)),
        Err(AmbientWeatherRefusal::NativeContract)
    );
    body["current"]["temperature_2m"] = 0.0001.into();
    assert_eq!(
        decoder.decode_reading(&encode(&body)),
        Err(AmbientWeatherRefusal::TemperaturePrecision)
    );
}

#[test]
fn original_decimal_precision_survives_json_float_normalization() {
    let decoder = prepared();
    let template = String::from_utf8(encode(&response())).unwrap();
    for (token, expected) in [("7e0", 7_000_i64), ("-2.1250", -2_125), ("700e-2", 7_000)] {
        let body = template.replace("-2.125", token);
        let reading = decoder.decode_reading(body.as_bytes()).unwrap();
        let actual: i64 = rust_binding::primitive_from_structured(
            field(&reading, "temperature_celsius_milli").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
    for token in ["7.0000000000000001", "1e-4", "-0.000000000000000001"] {
        let body = template.replace("-2.125", token);
        assert_eq!(
            decoder.decode_reading(body.as_bytes()),
            Err(AmbientWeatherRefusal::TemperaturePrecision)
        );
    }
    let escaped_key = template.replace("temperature_2m", "temperat\\u0075re_2m");
    assert!(decoder.decode_reading(escaped_key.as_bytes()).is_ok());
    let duplicate = template.replace(
        "\"weather_code\":63",
        "\"weather_code\":63,\"weather_code\":0",
    );
    assert_eq!(
        decoder.decode_reading(duplicate.as_bytes()),
        Err(AmbientWeatherRefusal::MalformedResponse)
    );
}

#[test]
fn time_overflow_and_negative_epoch_refuse_without_wrapping() {
    let decoder = prepared();
    let mut body = response();
    body["current"]["time"] = u64::MAX.into();
    assert_eq!(
        decoder.decode_reading(&encode(&body)),
        Err(AmbientWeatherRefusal::TimestampOverflow)
    );
    body["current"]["time"] = (-1).into();
    assert_eq!(
        decoder.decode_reading(&encode(&body)),
        Err(AmbientWeatherRefusal::MalformedResponse)
    );
}

#[test]
fn unavailable_is_explicit_bounded_and_never_fabricates_a_reading() {
    let decoder = prepared();
    let unavailable = decoder.unavailable("Weather provider unavailable").unwrap();
    assert_eq!(tag(&unavailable), "unavailable");
    assert_eq!(
        decoder.classify(&unavailable, 100_000),
        Err(AmbientWeatherRefusal::WrongReading)
    );
    assert_eq!(
        decoder.unavailable(""),
        Err(AmbientWeatherRefusal::NativeContract)
    );
    assert_eq!(
        decoder.unavailable(&"x".repeat(161)),
        Err(AmbientWeatherRefusal::NativeContract)
    );
    assert!(PreparedOpenMeteoWeather::prepare("", 5_000).is_err());
    assert!(PreparedOpenMeteoWeather::prepare("Seattle", 0).is_err());
    assert!(PreparedOpenMeteoWeather::prepare("Seattle", MAXIMUM_WEATHER_AGE_MS + 1).is_err());
}

#[test]
fn known_weather_codes_include_heavy_thunderstorm_and_never_default_unknown_to_clear() {
    for code in [
        0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82, 85,
        86, 95, 96, 97, 99,
    ] {
        assert!(condition(code).is_ok());
    }
    assert_eq!(condition(97), Ok("Thunderstorms, heavy"));
    assert_eq!(
        condition(100),
        Err(AmbientWeatherRefusal::UnknownWeatherCode(100))
    );
}

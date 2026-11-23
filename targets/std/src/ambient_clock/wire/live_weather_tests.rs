use super::*;
use crate::ambient_weather::PreparedCurrentWeatherWire;
use conduit_core::StructuredInfoValue;

fn assert_snapshot(
    clock: &mut PreparedAmbientClockWire,
    weather: &mut PreparedCurrentWeatherWire,
    now: u64,
) {
    let typed = super::super::PreparedAmbientClockWeather::prepare("America/Los_Angeles").unwrap();
    let weather_value =
        StructuredInfoValue::from_canonical_bytes(weather.encode(now).unwrap()).unwrap();
    let expected = typed
        .snapshot(now, &weather_value)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let backing = clock.prepared_heap_bytes() + weather.prepared_heap_bytes();
    let probe = crate::allocation_probe::begin();
    assert_eq!(
        clock.encode_with_current_weather(now, weather).unwrap(),
        expected
    );
    assert_eq!(probe.finish_bytes(), 0);
    assert_eq!(
        clock.prepared_heap_bytes() + weather.prepared_heap_bytes(),
        backing
    );
}
#[test]
fn live_weather_refreshes_feed_the_same_clock_without_replaying_freshness() {
    let mut clock = PreparedAmbientClockWire::prepare_live_weather("America/Los_Angeles").unwrap();
    let mut weather = PreparedCurrentWeatherWire::prepare("Seattle", 5_000).unwrap();
    assert_snapshot(&mut clock, &mut weather, 99_999);
    let body = r#"{"current_units":{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"},"current":{"time":100,"temperature_2m":12.5,"weather_code":0}}"#;
    weather.refresh(body.as_bytes(), 100_000).unwrap();
    for now in [100_000, 105_000, 105_001] {
        assert_snapshot(&mut clock, &mut weather, now);
    }
    assert!(weather.refresh(b"{}", 106_000).is_err());
    assert_snapshot(&mut clock, &mut weather, 106_000);
    let next = body
        .replace("12.5", "15.25")
        .replace("\"time\":100", "\"time\":107");
    weather.refresh(next.as_bytes(), 107_000).unwrap();
    assert_snapshot(&mut clock, &mut weather, 107_000);
}

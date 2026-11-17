use super::*;
use crate::ambient_wallpaper::{MAXIMUM_APOD_RESPONSE_BYTES, acquisition::*};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::rust_binding;
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use std::{
    cell::Cell,
    io::{Read, Write},
    net::TcpListener,
    thread,
};

// Tiny generated inputs exercise the actual preparation entrance; no recorded
// provider response, photograph or encoded binary fixture is retained in source.
fn deck() -> PreparedDeck {
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0u8;
    prepare_deck(admission, |url, bound| {
        if bound == MAXIMUM_APOD_RESPONSE_BYTES {
            Ok(serde_json::to_vec(&serde_json::json!({
                "date":url.rsplit('=').next().unwrap(), "title":"Prepared source photograph",
                "media_type":"image", "hdurl":"https://apod.nasa.gov/fixture.png",
                "copyright":"Fixture photographer"
            }))
            .unwrap())
        } else {
            number += 1;
            let mut bytes = Vec::new();
            PngEncoder::new(&mut bytes)
                .write_image(&[number, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
                .unwrap();
            Ok(bytes)
        }
    })
    .unwrap()
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag
}
fn checked_home(bytes: &[u8]) -> StructuredInfoValue {
    let home = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let types = conduit_semantic_catalog::prepare_ambient_display_types().unwrap();
    let ty = types
        .native_types
        .iter()
        .find(|t| t.name == "AmbientHomeObservation")
        .unwrap();
    assert_eq!(home.value_type(), &ty.value_type);
    rust_binding::validate_native_contracts(&home, &ty.value_contracts).unwrap();
    rust_binding::validate_native_invariants(&home, &ty.invariants).unwrap();
    assert_eq!(home.canonical_bytes().unwrap(), bytes);
    home
}
fn weather_body() -> Vec<u8> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 1;
    format!(r#"{{"current_units":{{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"}},"current":{{"time":{seconds},"temperature_2m":12.5,"weather_code":0}}}}"#).into_bytes()
}
fn server(status: u16, body: &[u8]) -> (SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response = format!(
        "HTTP/1.0 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let body = body.to_vec();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0u8; 1024];
        let mut length = 0;
        while !request[..length].windows(4).any(|p| p == b"\r\n\r\n") {
            let n = stream.read(&mut request[length..]).unwrap();
            assert_ne!(n, 0);
            length += n;
        }
        assert!(request[..length].starts_with(b"GET /v1/forecast?"));
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(&body).unwrap();
        // Listener and accepted connection close after this one response.
    });
    (address, server)
}
fn assert_clock_weather(home: &StructuredInfoValue, body: Option<&[u8]>) {
    let clock = field(home, "clock_weather");
    let now: u64 =
        rust_binding::primitive_from_structured(field(clock, "observed_at_unix_ms")).unwrap();
    let native =
        crate::ambient_weather::PreparedOpenMeteoWeather::prepare("Seattle", 900_000).unwrap();
    let weather = if let Some(body) = body {
        native
            .classify(&native.decode_reading(body).unwrap(), now)
            .unwrap()
    } else {
        native.unavailable("Weather provider unavailable").unwrap()
    };
    let expected =
        crate::ambient_clock::PreparedAmbientClockWeather::prepare("America/Los_Angeles")
            .unwrap()
            .snapshot(now, &weather)
            .unwrap();
    assert_eq!(clock, &expected);
    assert_eq!(tag(field(home, "wallpaper")), "available");
}

#[test]
fn real_http_weather_composes_checked_home_without_runtime_heap_growth_or_refetch() {
    let body = weather_body();
    let (endpoint, server) = server(200, &body);
    let mut provider =
        PreparedAmbientObservation::prepare("America/Los_Angeles", endpoint, deck(), 3).unwrap();
    let backing = provider.prepared_heap_bytes();
    let pointers = provider
        .photos()
        .photos()
        .each_ref()
        .map(|p| p.rgba().as_ptr());
    let identities = provider
        .photos()
        .photos()
        .each_ref()
        .map(|p| *p.original_sha256());
    let mut output = [0u8; 8192];
    let input = conduit_time::encode_tick(0);
    // Probe is current-thread only. Fixture server IO runs on its own thread;
    // preparation, fixture construction and allocating typed checks are outside.
    let probe = crate::allocation_probe::begin();
    let first = provider.observe(
        &input,
        &mut output,
        Instant::now() + Duration::from_secs(3),
        || false,
    );
    let statistics = probe.finish_statistics();
    let length = first.unwrap();
    assert_eq!(statistics.requested_bytes, 0);
    server.join().unwrap();
    assert_eq!(provider.last_transport_failure(), None);
    assert_clock_weather(&checked_home(&output[..length]), Some(&body));
    let attempted = provider.last_weather_attempt;
    let probe = crate::allocation_probe::begin();
    let second = provider.observe(
        &input,
        &mut output,
        Instant::now() + Duration::from_secs(3),
        || false,
    );
    let statistics = probe.finish_statistics();
    assert_eq!(statistics.requested_bytes, 0);
    assert_clock_weather(&checked_home(&output[..second.unwrap()]), Some(&body));
    assert_eq!(provider.last_weather_attempt, attempted);
    assert_eq!(provider.last_transport_failure(), None);
    assert_eq!(provider.prepared_heap_bytes(), backing);
    assert_eq!(
        provider
            .photos()
            .photos()
            .each_ref()
            .map(|p| p.rgba().as_ptr()),
        pointers
    );
    assert_eq!(
        provider
            .photos()
            .photos()
            .each_ref()
            .map(|p| *p.original_sha256()),
        identities
    );
}

#[test]
fn http_failure_still_emits_current_clock_and_explicit_unavailable_weather() {
    let (endpoint, server) = server(503, b"{}");
    let mut provider =
        PreparedAmbientObservation::prepare("America/Los_Angeles", endpoint, deck(), 1).unwrap();
    let mut output = [0u8; 8192];
    let probe = crate::allocation_probe::begin();
    let result = provider.observe(
        &conduit_time::encode_tick(0),
        &mut output,
        Instant::now() + Duration::from_secs(3),
        || false,
    );
    let statistics = probe.finish_statistics();
    assert_eq!(statistics.requested_bytes, 0);
    assert_clock_weather(&checked_home(&output[..result.unwrap()]), None);
    assert_eq!(
        provider.last_transport_failure(),
        Some(OpenMeteoHttpRefusal::Status(503))
    );
    server.join().unwrap();
    output.fill(0xA5);
    assert_eq!(
        provider.observe(
            &conduit_time::encode_tick(0),
            &mut output,
            Instant::now() + Duration::from_secs(3),
            || false
        ),
        Err(WorkerFailure::Sequence)
    );
    assert!(output.iter().all(|byte| *byte == 0xA5));
}

#[test]
fn cancellation_deadline_and_invalid_tick_preserve_output_and_update_budget() {
    let endpoint = "127.0.0.1:9".parse().unwrap();
    let mut provider =
        PreparedAmbientObservation::prepare("America/Los_Angeles", endpoint, deck(), 2).unwrap();
    let input = conduit_time::encode_tick(0);
    let mut output = [0xA5; 8192];
    let probe = crate::allocation_probe::begin();
    assert_eq!(
        provider.observe(
            &input,
            &mut output,
            Instant::now() + Duration::from_secs(1),
            || true
        ),
        Err(WorkerFailure::Cancelled)
    );
    assert_eq!(
        provider.observe(&input, &mut output, Instant::now(), || false),
        Err(WorkerFailure::TimedOut)
    );
    assert_eq!(
        provider.observe(
            b"bad",
            &mut output,
            Instant::now() + Duration::from_secs(1),
            || false
        ),
        Err(WorkerFailure::Provider)
    );
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    assert!(output.iter().all(|byte| *byte == 0xA5));
    assert_eq!(provider.updates, 0);
    assert_eq!(provider.last_weather_attempt, None);
    // Exercise cancellation after entry and immediately before composition,
    // without socket timing: a previously attempted refresh is not due again.
    provider.last_weather_attempt = Some(Instant::now());
    let calls = Cell::new(0);
    let probe = crate::allocation_probe::begin();
    let result = provider.observe(
        &input,
        &mut output,
        Instant::now() + Duration::from_secs(1),
        || {
            let n = calls.get() + 1;
            calls.set(n);
            n >= 2
        },
    );
    assert_eq!(result, Err(WorkerFailure::Cancelled));
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    assert!(output.iter().all(|byte| *byte == 0xA5));
    assert_eq!(provider.updates, 0);
}

#[test]
fn pure_home_projection_rotates_prepared_pixels_without_transport_or_allocation() {
    let mut provider = PreparedAmbientObservation::prepare(
        "America/Los_Angeles",
        "127.0.0.1:9".parse().unwrap(),
        deck(),
        2,
    )
    .unwrap();
    let backing = provider.prepared_heap_bytes();
    let mut output = [0u8; 8192];
    for now in [1_000, 301_000] {
        let probe = crate::allocation_probe::begin();
        let projected = provider.project(now).unwrap();
        let length = projected.len();
        output[..length].copy_from_slice(projected);
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
        let home = checked_home(&output[..length]);
        assert_clock_weather(&home, None);
        let selected = field(&home, "wallpaper");
        let StructuredInfoValueShape::Variant { payload, .. } = selected.shape() else {
            panic!("selection")
        };
        let selected_at: u64 =
            rust_binding::primitive_from_structured(field(payload, "selected_at_unix_ms")).unwrap();
        assert_eq!(selected_at, now);
        assert_eq!(provider.prepared_heap_bytes(), backing);
    }
    assert_eq!(provider.last_weather_attempt, None);
    assert_eq!(provider.updates, 0);
}

#[test]
fn resident_provider_crosses_4096_updates_without_growing_or_wrapping_identity() {
    let endpoint = "127.0.0.1:1".parse().unwrap();
    let mut provider =
        PreparedAmbientObservation::prepare_resident("America/Los_Angeles", endpoint, deck())
            .unwrap();
    // Deliberately skip HTTP within its real freshness interval. This tests
    // standing projection/custody, not live transport or provider availability.
    provider.last_weather_attempt = Some(Instant::now());
    let backing = provider.prepared_heap_bytes();
    let mut output = [0u8; 8192];
    let mut length = 0;
    let probe = crate::allocation_probe::begin();
    for sequence in 0..4100 {
        length = provider
            .observe(
                &conduit_time::encode_tick(sequence),
                &mut output,
                Instant::now() + Duration::from_secs(1),
                || false,
            )
            .unwrap();
    }
    assert_eq!(probe.finish_statistics().requested_bytes, 0);
    assert_eq!(provider.prepared_heap_bytes(), backing);
    provider.updates = u32::MAX;
    assert_eq!(
        provider.observe(
            &conduit_time::encode_tick(4100),
            &mut output,
            Instant::now() + Duration::from_secs(1),
            || false
        ),
        Err(WorkerFailure::Sequence)
    );
    checked_home(&output[..length]);
}

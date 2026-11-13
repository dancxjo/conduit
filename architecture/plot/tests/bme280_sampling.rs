use conduit_core::{ConfigurationValue, StructuredInfoType, StructuredInfoValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
#[path = "bme280_sampling/composition.rs"]
mod composition;
#[path = "bme280_sampling/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "bme280_lifecycle/fixture.rs"]
mod lifecycle;
use fixture::*;
use lifecycle::{byte, bytes, field, payload, tag};

fn lifecycle_catalog() -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
    for (source, aliases) in [
        (
            include_str!("../../../plots/device-protocols/i2c-types.conduit"),
            vec![
                ("I2cTransaction", "machine/i2c/transact/request"),
                ("I2cResult", "machine/i2c/transact/result"),
            ],
        ),
        (
            include_str!("../../../plots/device-protocols/clock-types.conduit"),
            vec![
                ("MonotonicClockResult", "machine/clock/at/result"),
                ("MonotonicClockRequest", "machine/clock/at/request"),
            ],
        ),
    ] {
        let checked =
            check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
        for (name, path) in aliases {
            let ty = checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap();
            catalog
                .insert_checked_native_type_from_document(path, &checked, &ty.identity)
                .unwrap();
        }
    }
    catalog
}

#[test]
fn two_complete_acquisitions_share_one_outer_lifecycle_and_wait_between_samples() {
    let mut f = Sampling::new();
    let mut observation = Observation::new();
    let mut state = f.start(2, 100);
    let first = f.acquire(&mut state, 0, 0);
    assert_eq!(number(field(&state, "remaining")), 1);
    assert_eq!(byte(field(&state, "terminal")), 0);
    assert_eq!(byte(field(&state, "captured")), 1);
    assert_eq!(tag(&f.route(&state)), "waiting");
    assert_eq!(bytes(field(&first, "sample")), SAMPLE);
    let due = number(field(&state, "next_due"));
    let early = f.protocol.tick(due - 1);
    state = f.advance(state, early);
    assert_eq!(byte(field(&state, "waiting")), 1);
    assert_eq!(byte(field(&state, "captured")), 0);
    let on_time = f.protocol.tick(due);
    state = f.advance(state, on_time);
    assert_eq!(byte(field(&state, "waiting")), 0);
    assert_eq!(byte(field(field(&state, "protocol"), "phase")), 0);
    let second = f.acquire(&mut state, due, 1);
    let mut expected = SAMPLE;
    expected[7] += 1;
    assert_eq!(bytes(field(&second, "sample")), expected);
    assert_eq!(bytes(field(&second, "first")), calibration());
    for (capture, humidity) in [(first, 55588_i128), (second, 55600_i128)] {
        let decoded = observation.decode(capture);
        assert_eq!(tag(&decoded), "observation");
        for (name, expected) in [
            ("temperature_centidegrees", 2508),
            ("pressure_q24_8_pa", 25767233),
            ("humidity_q22_10_percent", humidity),
        ] {
            let conduit_core::StructuredInfoValueShape::Leaf(encoded) =
                field(payload(&decoded), name).shape()
            else {
                panic!("i128")
            };
            assert_eq!(
                i128::from_le_bytes((*encoded).try_into().unwrap()),
                expected,
                "{name}"
            );
        }
    }
    assert_eq!(number(field(&state, "remaining")), 0);
    assert_eq!(byte(field(&state, "captured")), 1);
    assert_eq!(byte(field(&state, "terminal")), 1);
    assert_eq!(tag(&f.route(&state)), "stopped");
    assert_eq!(tag(field(&state, "failure")), "none");
}

#[test]
fn count_period_overflow_and_provider_refusals_are_finite_without_retry() {
    let mut f = Sampling::new();
    for (count, period, failure) in [(0, 1, "invalid-count"), (1, 0, "invalid-period")] {
        let state = f.start(count, period);
        assert_eq!(byte(field(&state, "terminal")), 1);
        assert_eq!(tag(field(&state, "failure")), failure);
        assert_eq!(tag(&f.route(&state)), "stopped");
    }
    let mut state = f.start(2, u64::MAX);
    f.acquire(&mut state, 0, 0);
    assert_eq!(tag(field(&state, "failure")), "deadline-overflow");
    assert_eq!(byte(field(&state, "captured")), 1);
    assert_eq!(tag(&f.route(&state)), "stopped");
    let mut last = f.start(1, u64::MAX);
    f.acquire(&mut last, 0, 0);
    assert_eq!(tag(field(&last, "failure")), "none");
    assert_eq!(number(field(&last, "remaining")), 0);
    let state = f.start(3, 100);
    let lost = f.protocol.bus("provider-lost", &[], 0);
    let failed = f.advance(state, lost);
    assert_eq!(tag(field(&failed, "failure")), "protocol");
    assert_eq!(tag(payload(&f.refusal(&failed))), "bus");
    assert_eq!(tag(&f.route(&failed)), "stopped");
    let mut reversed = f.start(2, 100);
    f.acquire(&mut reversed, 0, 0);
    let prior = number(field(field(&reversed, "protocol"), "clock"));
    let event = f.protocol.tick(prior - 1);
    let reversed = f.advance(reversed, event);
    assert_eq!(tag(field(&reversed, "failure")), "clock-reversed");
    assert_eq!(tag(&f.route(&reversed)), "stopped");
    let mut waiting = f.start(2, 100);
    f.acquire(&mut waiting, 0, 0);
    let lost = f.protocol.clock_failure("provider-lost");
    let failed = f.advance(waiting, lost);
    assert_eq!(tag(payload(&f.refusal(&failed))), "clock");
    assert_eq!(tag(&f.route(&failed)), "stopped");
}

fn record(
    ty: &StructuredInfoType,
    values: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(name, value)| conduit_core::StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}

#[test]
fn unsupported_address_stops_before_acquisition_and_matches_protocol_refusal() {
    let mut f = Sampling::new();
    let state = f.start_address(2, 100, 117);
    assert_eq!(byte(field(&state, "terminal")), 1);
    assert_eq!(byte(field(&state, "captured")), 0);
    assert_eq!(number(field(&state, "remaining")), 2);
    assert_eq!(tag(&f.route(&state)), "stopped");
    let failure = f.refusal(&state);
    assert_eq!(tag(&failure), "protocol");
    assert_eq!(tag(payload(&failure)), "unsupported-address");
    assert_eq!(byte(payload(payload(&failure))), 117);
    let action = f.protocol.action(field(&state, "protocol"));
    assert_eq!(tag(&action), "refused");
    assert_eq!(payload(&action), payload(&failure));
}

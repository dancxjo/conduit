use super::fixture::*;
use conduit_core::StructuredInfoValue;
fn request(f: &mut Fixture, state: &StructuredInfoValue, write: &[u8], read: u8) {
    let action = f.action(state);
    assert_eq!(tag(&action), "transact");
    let request = payload(&action);
    assert_eq!(byte(field(request, "address")), 0x76);
    assert_eq!(bytes(field(request, "write")), write);
    assert_eq!(byte(field(request, "read")), read);
}
#[test]
fn one_event_advances_one_phase_and_deadlines_use_monotonic_time() {
    let mut f = Fixture::new();
    assert!(f.checked_plot_count() > 10);
    let mut state = f.initial();
    request(&mut f, &state, &[0xd0], 1);
    state = f.advance(state, f.bus("completed", &[0x60], 0));
    assert_eq!(byte(field(&state, "phase")), 1);
    request(&mut f, &state, &[0xe0, 0xb6], 0);
    state = f.advance(state, f.bus("completed", &[], 1));
    assert_eq!(byte(field(&state, "phase")), 2);
    assert_eq!(tag(&f.action(&state)), "waiting");
    state = f.advance(state, f.tick(2));
    assert_eq!(byte(field(&state, "phase")), 2);
    state = f.advance(state, f.tick(3));
    assert_eq!(byte(field(&state, "phase")), 3);
    request(&mut f, &state, &[0xf3], 1);
    state = f.advance(state, f.bus("completed", &[0], 3));
    for write in [[0xf4, 0], [0xf2, 1], [0xf5, 0]] {
        request(&mut f, &state, &write, 0);
        state = f.advance(state, f.bus("completed", &[], 3));
    }
    let mut calibration = Vec::new();
    for coefficient in [
        27504_i32, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
    ] {
        calibration.extend_from_slice(&(coefficient as u16).to_le_bytes());
    }
    for (index, value) in calibration.iter().enumerate() {
        request(&mut f, &state, &[0x88 + index as u8], 1);
        state = f.advance(state, f.bus("completed", &[*value], 3));
    }
    assert_eq!(bytes(field(&state, "first")), calibration);
    request(&mut f, &state, &[0xa1], 1);
    state = f.advance(state, f.bus("completed", &[75], 3));
    let humidity = [0x6a, 1, 0, 0x13, 0x2b, 3, 30];
    for (index, value) in humidity.iter().enumerate() {
        request(&mut f, &state, &[0xe1 + index as u8], 1);
        state = f.advance(state, f.bus("completed", &[*value], 3));
    }
    assert_eq!(bytes(field(&state, "humidity")), humidity);
    request(&mut f, &state, &[0xf4, 0x25], 0);
    state = f.advance(state, f.bus("completed", &[], 4));
    state = f.advance(state, f.tick(23));
    assert_eq!(tag(&f.action(&state)), "waiting");
    state = f.advance(state, f.tick(24));
    request(&mut f, &state, &[0xf3], 1);
    state = f.advance(state, f.bus("completed", &[0], 24));
    let sample = [0x65, 0x5a, 0xc0, 0x7e, 0xed, 0, 0x75, 0x30];
    for (index, value) in sample.iter().enumerate() {
        request(&mut f, &state, &[0xf7 + index as u8], 1);
        state = f.advance(state, f.bus("completed", &[*value], 24));
    }
    assert_eq!(tag(&f.action(&state)), "captured");
    assert_eq!(bytes(field(&state, "sample")), sample);
    assert_eq!(tag(field(&state, "failure")), "none");
    state = f.advance(state, f.tick(25));
    assert_eq!(tag(&f.action(&state)), "waiting");
}
#[test]
fn identity_short_bus_loss_and_unexpected_events_refuse_without_new_requests() {
    for (bus, data, wanted) in [
        ("completed", vec![0x58], "wrong-identity"),
        ("short", vec![], "bus"),
        ("provider-lost", vec![], "bus"),
        ("completed", vec![], "malformed"),
        ("completed", vec![0x60, 0], "malformed"),
    ] {
        let mut f = Fixture::new();
        let initial = f.initial();
        let state = f.advance(initial, f.bus(bus, &data, 0));
        assert_eq!(tag(field(&state, "failure")), wanted);
        let action = f.action(&state);
        assert_eq!(tag(&action), "refused");
        if wanted == "bus" {
            assert_eq!(tag(payload(field(&state, "failure"))), bus);
        }
    }
    let mut f = Fixture::new();
    let initial = f.initial();
    let state = f.advance(initial, f.bus("completed", &[0x60], 0));
    let state = f.advance(state, f.bus("completed", &[], 10));
    let state = f.advance(state, f.bus("completed", &[], 11));
    assert_eq!(tag(field(&state, "failure")), "unexpected");
    assert_eq!(tag(&f.action(&state)), "refused");
}
#[test]
fn polling_exhaustion_and_stale_clock_are_explicit() {
    let mut f = Fixture::new();
    let initial = f.initial();
    let mut state = f.with_phase(&initial, 3);
    for index in 0..8 {
        request(&mut f, &state, &[0xf3], 1);
        state = f.advance(state, f.bus("completed", &[1], 10));
        assert_eq!(
            byte(field(&state, "phase")),
            if index == 7 { 15 } else { 3 }
        );
    }
    assert_eq!(tag(field(&state, "failure")), "timeout");
    let initial = f.initial();
    let state = f.advance(initial, f.bus("completed", &[0x60], 10));
    let state = f.advance(state, f.tick(9));
    assert_eq!(tag(field(&state, "failure")), "malformed");
}

#[test]
fn clock_events_do_not_repeat_an_in_flight_transaction() {
    let mut f = Fixture::new();
    let initial = f.initial();
    request(&mut f, &initial, &[0xd0], 1);
    let mut state = f.advance(initial, f.tick(1));
    assert_eq!(tag(&f.action(&state)), "waiting");
    state = f.advance(state, f.bus("completed", &[0x60], 2));
    request(&mut f, &state, &[0xe0, 0xb6], 0);
    state = f.advance(state, f.tick(3));
    assert_eq!(tag(&f.action(&state)), "waiting");
    state = f.advance(state, f.bus("completed", &[], 4));
    state = f.advance(state, f.tick(6));
    request(&mut f, &state, &[0xf3], 1);
}

#[test]
fn prepared_transition_reuse_never_allocates_or_grows() {
    let mut f = Fixture::new();
    let initial = f.initial();
    let input = f.input_bytes(initial, f.bus("completed", &[0x60], 1));
    let (_, observation) = super::allocation_probe::observe(|| {
        for _ in 0..1_000 {
            assert!(!f.advance_bytes(&input).is_empty());
        }
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
}

#[test]
fn source_initializer_accepts_only_the_two_protocol_addresses() {
    let mut f = Fixture::new();
    for address in [0x76, 0x77] {
        let state = f.initial_address(address);
        let action = f.action(&state);
        assert_eq!(tag(&action), "transact");
        assert_eq!(byte(field(payload(&action), "address")), address);
        assert_eq!(bytes(field(payload(&action), "write")), [0xd0]);
    }
    let state = f.initial_address(0x75);
    let action = f.action(&state);
    assert_eq!(tag(&action), "refused");
    assert_eq!(tag(payload(&action)), "unsupported-address");
}

#[test]
fn waiting_retains_observed_time_and_rejects_a_later_stale_event() {
    let mut f = Fixture::new();
    let initial = f.initial();
    let state = f.advance(initial, f.bus("completed", &[0x60], 0));
    let state = f.advance(state, f.bus("completed", &[], 10));
    let state = f.advance(state, f.tick(11));
    assert_eq!(tag(&f.action(&state)), "waiting");
    let state = f.advance(state, f.tick(10));
    assert_eq!(tag(field(&state, "failure")), "malformed");
    assert_eq!(tag(&f.action(&state)), "refused");
}

use super::{
    decode_signal, decode_signal_bytes, encode_signal, parse_pulse_configuration,
    pulse_configuration_entries, PulseConfiguration, Signal,
};

#[test]
fn round_trips_signal_payload() {
    let payload = encode_signal(&Signal {
        sequence: 7,
        level: true,
    });
    let decoded = decode_signal(&payload).expect("signal payload should decode");
    assert_eq!(decoded.sequence, 7);
    assert!(decoded.level);
    assert_eq!(
        decode_signal_bytes(&payload.encoded).expect("fixed bytes should decode"),
        decoded
    );
}

#[test]
fn round_trips_pulse_configuration_entries() {
    let config = PulseConfiguration {
        count: 3,
        period_ms: 0,
        initial_level: false,
    };
    let parsed = parse_pulse_configuration(&pulse_configuration_entries(&config))
        .expect("pulse configuration should parse");
    assert_eq!(parsed, config);
}

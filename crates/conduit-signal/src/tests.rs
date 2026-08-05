use super::{
    decode_signal_bytes, decode_signal_fixed, encode_signal_fixed, encode_signal_into,
    signal_level_for_sequence, Signal, SIGNAL_ENCODED_LEN_USIZE,
};

#[cfg(feature = "host-profile")]
use super::{
    decode_signal, encode_signal, parse_pulse_configuration, pulse_configuration_entries,
    PulseConfiguration, PICO_LOCAL_BOOT_ID, PICO_LOCAL_HOST_ID,
};

#[cfg(feature = "host-profile")]
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
fn fixed_signal_helpers_do_not_require_payload_allocation() {
    let signal = Signal {
        sequence: 9,
        level: true,
    };
    let encoded = encode_signal_fixed(&signal);
    assert_eq!(encoded.len(), SIGNAL_ENCODED_LEN_USIZE);
    assert_eq!(decode_signal_fixed(&encoded), signal);
    assert_eq!(
        decode_signal_bytes(&encoded).expect("fixed bytes should decode"),
        signal
    );

    let mut output = [0u8; SIGNAL_ENCODED_LEN_USIZE];
    encode_signal_into(&signal, &mut output).expect("fixed buffer accepts signal");
    assert_eq!(output, encoded);
    assert_eq!(
        encode_signal_into(&signal, &mut output[..SIGNAL_ENCODED_LEN_USIZE - 1]),
        Err(super::SignalProfileError::WrongEncodedLength(
            SIGNAL_ENCODED_LEN_USIZE - 1
        ))
    );
}

#[test]
fn signal_level_matches_portable_sixteen_value_profile() {
    let levels = core::array::from_fn(|sequence| signal_level_for_sequence(sequence as u64, false));
    assert_eq!(
        levels,
        [
            false, true, false, true, false, true, false, true, false, true, false, true, false,
            true, false, true
        ]
    );
}

#[cfg(feature = "host-profile")]
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

#[cfg(feature = "host-profile")]
#[test]
fn pico_local_advertisement_names_exact_constrained_signal_profile() {
    let advertisement = super::pico_local_advertisement();
    assert_eq!(advertisement.host_id.as_str(), PICO_LOCAL_HOST_ID);
    assert_eq!(advertisement.boot_id.as_str(), PICO_LOCAL_BOOT_ID);
    assert_eq!(advertisement.capabilities.len(), 2);
    assert_eq!(advertisement.resources.len(), 2);
    assert!(advertisement
        .capabilities
        .iter()
        .all(|capability| capability.limits.max_active_instances == 1
            && capability.limits.max_queue_items == 1
            && capability.limits.max_queue_bytes == super::SIGNAL_ENCODED_LEN));
    assert!(advertisement.capabilities.iter().any(|capability| {
        capability.kind_id == super::pulse_kind()
            && capability.outputs == super::pulse_outputs()
            && capability.inputs.is_empty()
    }));
    assert!(advertisement.capabilities.iter().any(|capability| {
        capability.kind_id == super::show_kind()
            && capability.inputs == super::show_inputs()
            && capability.outputs.is_empty()
    }));
}

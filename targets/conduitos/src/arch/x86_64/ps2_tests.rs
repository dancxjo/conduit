use super::*;

#[test]
fn set_two_make_break_maps_directly_to_portable_usage() {
    let mut decoder = KeyboardDecoder::new();
    let press = decoder.accept(0x1c).unwrap().unwrap();
    assert_eq!(
        (press.usage(), press.pressed(), press.modifiers()),
        (4, true, 0)
    );
    assert_eq!(decoder.accept(0xf0).unwrap(), None);
    let release = decoder.accept(0x1c).unwrap().unwrap();
    assert_eq!((release.usage(), release.pressed()), (4, false));
}

#[test]
fn modifier_state_is_after_each_transition() {
    let mut decoder = KeyboardDecoder::new();
    let control = decoder.accept(0x14).unwrap().unwrap();
    assert_eq!(control.modifiers(), 1);
    assert_eq!(decoder.accept(0xf0).unwrap(), None);
    assert_eq!(decoder.accept(0x14).unwrap().unwrap().modifiers(), 0);
}

#[test]
fn relative_pointer_packet_preserves_only_truthful_semantics() {
    let mut decoder = PointerDecoder::new();
    assert_eq!(decoder.accept(0x09).unwrap(), None);
    assert_eq!(decoder.accept(2).unwrap(), None);
    let sample = decoder.accept(1).unwrap().unwrap();
    assert_eq!((sample.delta_x, sample.delta_y), (8_000, -4_000));
    assert!(sample.primary_pressed);
    assert_eq!(sample.sequence, 1);
}

#[test]
fn pending_pointer_coalesces_one_hundred_thousand_samples_in_one_slot() {
    let mut input = Ps2Input {
        keyboard: KeyboardDecoder::new(),
        pointer: PointerDecoder::new(),
        pending_keyboard: None,
        pending_pointer: None,
    };
    for sequence in 1..=100_000 {
        input
            .retain_pointer(NormalizedPointerSample {
                position_x: sequence,
                position_y: 1_000_000 - sequence,
                delta_x: 1,
                delta_y: -1,
                primary_pressed: sequence % 2 == 0,
                coalesced: 0,
                dropped: 0,
                queue_capacity: 1,
                sequence: sequence as u64,
            })
            .unwrap();
    }
    let sample = input.pending_pointer.take().unwrap();
    assert_eq!((sample.position_x, sample.position_y), (100_000, 900_000));
    assert_eq!((sample.delta_x, sample.delta_y), (100_000, -100_000));
    assert!(sample.primary_pressed);
    assert_eq!(sample.sequence, 100_000);
    assert_eq!(sample.coalesced, 99_999);
    assert_eq!(sample.dropped, 0);
    assert_eq!(sample.queue_capacity, 1);
}

#[test]
fn malformed_and_unsupported_input_refuse() {
    let mut keyboard = KeyboardDecoder::new();
    assert_eq!(keyboard.accept(0), Err(Ps2Error::UnsupportedKeyboardCode));
    let mut pointer = PointerDecoder::new();
    assert_eq!(pointer.accept(0), Err(Ps2Error::MalformedPointerPacket));
}

#[test]
fn coalesced_pointer_evidence_overflow_refuses_without_relabeling_sequence() {
    let mut input = Ps2Input {
        keyboard: KeyboardDecoder::new(),
        pointer: PointerDecoder::new(),
        pending_keyboard: None,
        pending_pointer: Some(NormalizedPointerSample {
            position_x: 0,
            position_y: 0,
            delta_x: i64::MAX,
            delta_y: 0,
            primary_pressed: false,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 1,
            sequence: 1,
        }),
    };
    let error = input.retain_pointer(NormalizedPointerSample {
        position_x: 1,
        position_y: 1,
        delta_x: 1,
        delta_y: 0,
        primary_pressed: true,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 1,
        sequence: 2,
    });
    assert_eq!(error, Err(Ps2Error::PressureEvidenceOverflow));
    assert_eq!(input.pending_pointer.unwrap().sequence, 1);
}

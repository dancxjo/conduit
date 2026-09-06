#[path = "../../../targets/rp2040/firmware/pico-w-signal/src/midi_fixture_mapping.rs"]
mod mapping;

use mapping::*;

fn scan_three(
    state: &mut FixtureMapping,
    buttons: [bool; BUTTON_COUNT],
    adc: [u16; ANALOG_COUNT],
) -> Vec<UsbMidiPacket> {
    let mut output = [UsbMidiPacket([0; 4]); MAXIMUM_EVENTS_PER_SCAN];
    let mut result = Vec::new();
    for _ in 0..DEBOUNCE_SCANS {
        let count = state.scan(buttons, adc, &mut output);
        result.extend_from_slice(&output[..count]);
    }
    result
}

#[test]
fn eight_notes_and_sustain_are_debounced_and_exact() {
    let mut state = FixtureMapping::new();
    let initial = scan_three(&mut state, [false; BUTTON_COUNT], [0, 4095]);
    assert_eq!(
        initial,
        [
            UsbMidiPacket([0x0b, 0xb0, 1, 0]),
            UsbMidiPacket([0x0b, 0xb0, 11, 127])
        ]
    );
    assert_eq!(initial[0].bytes(), [0x0b, 0xb0, 1, 0]);

    for (index, key) in NOTE_KEYS.into_iter().enumerate() {
        let mut pressed = [false; BUTTON_COUNT];
        pressed[index] = true;
        assert_eq!(
            scan_three(&mut state, pressed, [0, 4095]),
            [UsbMidiPacket([0x09, 0x90, key, NOTE_VELOCITY])]
        );
        assert_eq!(
            scan_three(&mut state, [false; BUTTON_COUNT], [0, 4095]),
            [UsbMidiPacket([0x08, 0x80, key, 0])]
        );
    }

    let mut sustain = [false; BUTTON_COUNT];
    sustain[8] = true;
    assert_eq!(
        scan_three(&mut state, sustain, [0, 4095]),
        [UsbMidiPacket([0x0b, 0xb0, 64, 127])]
    );
    assert_eq!(
        scan_three(&mut state, [false; BUTTON_COUNT], [0, 4095]),
        [UsbMidiPacket([0x0b, 0xb0, 64, 0])]
    );
}

#[test]
fn analog_controls_are_bounded_quantized_and_change_suppressed() {
    assert_eq!(quantize_adc(0), 0);
    assert_eq!(quantize_adc(4095), 127);
    assert_eq!(quantize_adc(u16::MAX), 127);
    let mut state = FixtureMapping::new();
    let mut output = [UsbMidiPacket([0; 4]); MAXIMUM_EVENTS_PER_SCAN];
    assert_eq!(
        state.scan([false; BUTTON_COUNT], [2048, 1024], &mut output),
        2
    );
    assert_eq!(
        output[0],
        UsbMidiPacket([0x0b, 0xb0, MODULATION_CONTROL, 64])
    );
    assert_eq!(
        output[1],
        UsbMidiPacket([0x0b, 0xb0, EXPRESSION_CONTROL, 32])
    );
    assert_eq!(
        state.scan([false; BUTTON_COUNT], [2050, 1025], &mut output),
        0
    );
}

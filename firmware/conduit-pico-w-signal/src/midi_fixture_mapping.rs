//! Pure finite control-to-USB-MIDI mapping for the independent Pico fixture.

pub const BUTTON_COUNT: usize = 9;
pub const ANALOG_COUNT: usize = 2;
pub const MAXIMUM_EVENTS_PER_SCAN: usize = BUTTON_COUNT + ANALOG_COUNT;
pub const NOTE_KEYS: [u8; 8] = [60, 62, 64, 65, 67, 69, 71, 72];
pub const SUSTAIN_CONTROL: u8 = 64;
pub const MODULATION_CONTROL: u8 = 1;
pub const EXPRESSION_CONTROL: u8 = 11;
pub const NOTE_VELOCITY: u8 = 100;
pub const DEBOUNCE_SCANS: u8 = 3;
pub const ANALOG_CHANGE_THRESHOLD: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsbMidiPacket(pub [u8; 4]);

impl UsbMidiPacket {
    pub const fn bytes(self) -> [u8; 4] {
        self.0
    }
}

#[derive(Clone, Copy)]
struct DebouncedButton {
    stable: bool,
    candidate: bool,
    candidate_scans: u8,
}

impl DebouncedButton {
    const fn new() -> Self {
        Self {
            stable: false,
            candidate: false,
            candidate_scans: 0,
        }
    }

    fn update(&mut self, pressed: bool) -> Option<bool> {
        if pressed == self.stable {
            self.candidate = pressed;
            self.candidate_scans = 0;
            return None;
        }
        if pressed != self.candidate {
            self.candidate = pressed;
            self.candidate_scans = 1;
            return None;
        }
        self.candidate_scans = self.candidate_scans.saturating_add(1);
        if self.candidate_scans < DEBOUNCE_SCANS {
            return None;
        }
        self.stable = pressed;
        self.candidate_scans = 0;
        Some(pressed)
    }
}

pub struct FixtureMapping {
    buttons: [DebouncedButton; BUTTON_COUNT],
    analog: [Option<u8>; ANALOG_COUNT],
}

impl FixtureMapping {
    pub const fn new() -> Self {
        Self {
            buttons: [DebouncedButton::new(); BUTTON_COUNT],
            analog: [None; ANALOG_COUNT],
        }
    }

    pub fn scan(
        &mut self,
        pressed: [bool; BUTTON_COUNT],
        adc: [u16; ANALOG_COUNT],
        output: &mut [UsbMidiPacket; MAXIMUM_EVENTS_PER_SCAN],
    ) -> usize {
        let mut length = 0;
        for (index, pressed) in pressed.into_iter().enumerate() {
            let Some(pressed) = self.buttons[index].update(pressed) else {
                continue;
            };
            output[length] = if index < NOTE_KEYS.len() {
                note_packet(NOTE_KEYS[index], pressed)
            } else {
                control_packet(SUSTAIN_CONTROL, if pressed { 127 } else { 0 })
            };
            length += 1;
        }
        for (index, sample) in adc.into_iter().enumerate() {
            let value = quantize_adc(sample);
            if self.analog[index]
                .is_some_and(|prior| prior.abs_diff(value) < ANALOG_CHANGE_THRESHOLD)
            {
                continue;
            }
            self.analog[index] = Some(value);
            let control = [MODULATION_CONTROL, EXPRESSION_CONTROL][index];
            output[length] = control_packet(control, value);
            length += 1;
        }
        length
    }
}

impl Default for FixtureMapping {
    fn default() -> Self {
        Self::new()
    }
}

pub const fn quantize_adc(sample: u16) -> u8 {
    let bounded = if sample > 4095 { 4095 } else { sample };
    ((bounded as u32 * 127 + 2047) / 4095) as u8
}

const fn note_packet(key: u8, pressed: bool) -> UsbMidiPacket {
    if pressed {
        UsbMidiPacket([0x09, 0x90, key, NOTE_VELOCITY])
    } else {
        UsbMidiPacket([0x08, 0x80, key, 0])
    }
}

const fn control_packet(control: u8, value: u8) -> UsbMidiPacket {
    UsbMidiPacket([0x0b, 0xb0, control, value])
}

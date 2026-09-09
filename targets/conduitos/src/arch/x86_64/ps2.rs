//! Bounded i8042 keyboard and relative-pointer realization.
//!
//! Scan codes and PS/2 packets end here. Valid keyboard transitions use the
//! repository's host-neutral keyboard usage vocabulary; pointer packets become
//! the existing normalized relative-pointer value directly.

use conduit_semantic_catalog::NormalizedPointerSample;

use super::{
    HidKeyTransition,
    io::{inb, outb},
};

const DATA: u16 = 0x60;
const STATUS_COMMAND: u16 = 0x64;
const OUTPUT_FULL: u8 = 1 << 0;
const INPUT_FULL: u8 = 1 << 1;
const AUXILIARY: u8 = 1 << 5;
const WAIT_STEPS: u32 = 100_000;
const POINTER_SCALE: i64 = 4_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ps2Error {
    ControllerTimeout,
    ControllerSelfTest,
    KeyboardPortAbsent,
    PointerPortAbsent,
    KeyboardResponse,
    PointerResponse,
    MalformedKeyboardCode,
    UnsupportedKeyboardCode,
    MalformedPointerPacket,
    KeyboardQueuePressure,
    PointerQueuePressure,
    SequenceOverflow,
}

impl Ps2Error {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ControllerTimeout => "ps2-controller-timeout",
            Self::ControllerSelfTest => "ps2-controller-self-test-failed",
            Self::KeyboardPortAbsent => "ps2-keyboard-port-absent",
            Self::PointerPortAbsent => "ps2-pointer-port-absent",
            Self::KeyboardResponse => "ps2-keyboard-response-invalid",
            Self::PointerResponse => "ps2-pointer-response-invalid",
            Self::MalformedKeyboardCode => "ps2-keyboard-code-malformed",
            Self::UnsupportedKeyboardCode => "ps2-keyboard-code-unsupported",
            Self::MalformedPointerPacket => "ps2-pointer-packet-malformed",
            Self::KeyboardQueuePressure => "ps2-keyboard-queue-pressure",
            Self::PointerQueuePressure => "ps2-pointer-queue-pressure",
            Self::SequenceOverflow => "ps2-pointer-sequence-overflow",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ps2Ready {
    pub controller_slots: u16,
    pub keyboard_transition_slots: u16,
    pub pointer_packet_slots: u16,
    pub operation_slots: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KeyboardDecoder {
    break_pending: bool,
    extended_pending: bool,
    modifiers: u8,
}

impl KeyboardDecoder {
    const fn new() -> Self {
        Self {
            break_pending: false,
            extended_pending: false,
            modifiers: 0,
        }
    }

    fn accept(&mut self, byte: u8) -> Result<Option<HidKeyTransition>, Ps2Error> {
        match byte {
            0xe0 if !self.extended_pending => {
                self.extended_pending = true;
                return Ok(None);
            }
            0xf0 if !self.break_pending => {
                self.break_pending = true;
                return Ok(None);
            }
            0xe0 | 0xf0 => return Err(Ps2Error::MalformedKeyboardCode),
            _ => {}
        }
        let usage =
            set2_usage(self.extended_pending, byte).ok_or(Ps2Error::UnsupportedKeyboardCode)?;
        let pressed = !self.break_pending;
        self.break_pending = false;
        self.extended_pending = false;
        if let Some(bit) = modifier_bit(usage) {
            if pressed {
                self.modifiers |= bit;
            } else {
                self.modifiers &= !bit;
            }
        }
        Ok(Some(HidKeyTransition::from_validated_physical(
            usage,
            pressed,
            self.modifiers,
        )))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PointerDecoder {
    bytes: [u8; 3],
    count: usize,
    x: i64,
    y: i64,
    sequence: u64,
}

impl PointerDecoder {
    const fn new() -> Self {
        Self {
            bytes: [0; 3],
            count: 0,
            x: 500_000,
            y: 500_000,
            sequence: 0,
        }
    }

    fn accept(&mut self, byte: u8) -> Result<Option<NormalizedPointerSample>, Ps2Error> {
        if self.count == 0 && byte & 0x08 == 0 {
            return Err(Ps2Error::MalformedPointerPacket);
        }
        self.bytes[self.count] = byte;
        self.count += 1;
        if self.count != 3 {
            return Ok(None);
        }
        self.count = 0;
        let header = self.bytes[0];
        if header & 0xc0 != 0 {
            return Err(Ps2Error::MalformedPointerPacket);
        }
        let dx = i64::from(self.bytes[1] as i8) * POINTER_SCALE;
        let dy = -(i64::from(self.bytes[2] as i8) * POINTER_SCALE);
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Ps2Error::SequenceOverflow)?;
        self.x = (self.x + dx).clamp(0, 1_000_000);
        self.y = (self.y + dy).clamp(0, 1_000_000);
        Ok(Some(NormalizedPointerSample {
            sequence: self.sequence,
            position_x: self.x,
            position_y: self.y,
            delta_x: dx,
            delta_y: dy,
            primary_pressed: header & 1 != 0,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 1,
        }))
    }
}

pub struct Ps2Input {
    keyboard: KeyboardDecoder,
    pointer: PointerDecoder,
    pending_keyboard: Option<HidKeyTransition>,
    pending_pointer: Option<NormalizedPointerSample>,
}

impl Ps2Input {
    pub fn initialize() -> Result<(Self, Ps2Ready), Ps2Error> {
        command(0xad)?;
        command(0xa7)?;
        flush_output();
        command(0xaa)?;
        if read(false)? != 0x55 {
            return Err(Ps2Error::ControllerSelfTest);
        }
        command(0x20)?;
        let configuration = read(false)?;
        command(0x60)?;
        // Polling owns both finite device streams. Keep controller IRQs and
        // legacy set-1 translation disabled, and both port clocks enabled.
        write_data(configuration & !(0x01 | 0x02 | 0x10 | 0x20 | 0x40))?;
        command(0xab)?;
        if read(false)? != 0 {
            return Err(Ps2Error::KeyboardPortAbsent);
        }
        command(0xa9)?;
        if read(false)? != 0 {
            return Err(Ps2Error::PointerPortAbsent);
        }
        command(0xae)?;
        command(0xa8)?;
        device_command(false, 0xff)?;
        expect(false, 0xaa, Ps2Error::KeyboardResponse)?;
        device_command(false, 0xf0)?;
        device_command(false, 0x02)?;
        device_command(true, 0xff)?;
        expect(true, 0xaa, Ps2Error::PointerResponse)?;
        expect(true, 0x00, Ps2Error::PointerResponse)?;
        device_command(true, 0xf4)?;
        Ok((
            Self {
                keyboard: KeyboardDecoder::new(),
                pointer: PointerDecoder::new(),
                pending_keyboard: None,
                pending_pointer: None,
            },
            Ps2Ready {
                controller_slots: 1,
                keyboard_transition_slots: 8,
                pointer_packet_slots: 1,
                operation_slots: 1,
            },
        ))
    }

    pub fn receive_keyboard(&mut self) -> Result<HidKeyTransition, Ps2Error> {
        loop {
            if let Some(transition) = self.poll_keyboard()? {
                return Ok(transition);
            }
        }
    }

    /// Read at most one controller byte, retaining decoder prefixes between
    /// calls so other admitted Host work can run while input is incomplete.
    pub fn poll_keyboard(&mut self) -> Result<Option<HidKeyTransition>, Ps2Error> {
        if let Some(transition) = self.pending_keyboard.take() {
            return Ok(Some(transition));
        }
        let Some((auxiliary, byte)) = try_read_any() else {
            return Ok(None);
        };
        if auxiliary {
            if let Some(sample) = self.pointer.accept(byte)?
                && self.pending_pointer.replace(sample).is_some()
            {
                return Err(Ps2Error::PointerQueuePressure);
            }
            return Ok(None);
        }
        self.keyboard.accept(byte)
    }

    pub fn receive_pointer(&mut self) -> Result<NormalizedPointerSample, Ps2Error> {
        if let Some(sample) = self.pending_pointer.take() {
            return Ok(sample);
        }
        loop {
            let Some((auxiliary, byte)) = try_read_any() else {
                continue;
            };
            if !auxiliary {
                if let Some(transition) = self.keyboard.accept(byte)?
                    && self.pending_keyboard.replace(transition).is_some()
                {
                    // Ordered transitions have one explicit cross-operation
                    // slot; further input is pressure, never a silent drop.
                    return Err(Ps2Error::KeyboardQueuePressure);
                }
                continue;
            }
            if let Some(value) = self.pointer.accept(byte)? {
                return Ok(value);
            }
        }
    }
}

fn wait_write() -> Result<(), Ps2Error> {
    for _ in 0..WAIT_STEPS {
        if unsafe { inb(STATUS_COMMAND) } & INPUT_FULL == 0 {
            return Ok(());
        }
    }
    Err(Ps2Error::ControllerTimeout)
}

fn command(value: u8) -> Result<(), Ps2Error> {
    wait_write()?;
    unsafe { outb(STATUS_COMMAND, value) };
    Ok(())
}

fn write_data(value: u8) -> Result<(), Ps2Error> {
    wait_write()?;
    unsafe { outb(DATA, value) };
    Ok(())
}

fn device_command(auxiliary: bool, value: u8) -> Result<(), Ps2Error> {
    if auxiliary {
        command(0xd4)?;
    }
    write_data(value)?;
    expect(
        auxiliary,
        0xfa,
        if auxiliary {
            Ps2Error::PointerResponse
        } else {
            Ps2Error::KeyboardResponse
        },
    )
}

fn expect(auxiliary: bool, value: u8, error: Ps2Error) -> Result<(), Ps2Error> {
    let (actual_auxiliary, actual) = read_any()?;
    if actual_auxiliary != auxiliary || actual != value {
        return Err(error);
    }
    Ok(())
}

fn read(auxiliary: bool) -> Result<u8, Ps2Error> {
    let (actual_auxiliary, value) = read_any()?;
    if actual_auxiliary != auxiliary {
        return Err(Ps2Error::ControllerSelfTest);
    }
    Ok(value)
}

fn read_any() -> Result<(bool, u8), Ps2Error> {
    try_read_any().ok_or(Ps2Error::ControllerTimeout)
}

fn try_read_any() -> Option<(bool, u8)> {
    for _ in 0..WAIT_STEPS {
        let status = unsafe { inb(STATUS_COMMAND) };
        if status & OUTPUT_FULL != 0 {
            return Some((status & AUXILIARY != 0, unsafe { inb(DATA) }));
        }
    }
    None
}

fn flush_output() {
    for _ in 0..32 {
        if unsafe { inb(STATUS_COMMAND) } & OUTPUT_FULL == 0 {
            break;
        }
        let _ = unsafe { inb(DATA) };
    }
}

const fn modifier_bit(usage: u8) -> Option<u8> {
    if usage >= 0xe0 && usage <= 0xe7 {
        Some(1 << (usage - 0xe0))
    } else {
        None
    }
}

const fn set2_usage(extended: bool, code: u8) -> Option<u8> {
    if extended {
        return match code {
            0x11 => Some(0xe6),
            0x14 => Some(0xe4),
            0x1f => Some(0xe3),
            0x27 => Some(0xe7),
            0x4a => Some(0x54),
            0x5a => Some(0x58),
            0x69 => Some(0x4d),
            0x6b => Some(0x50),
            0x6c => Some(0x4a),
            0x70 => Some(0x49),
            0x71 => Some(0x4c),
            0x72 => Some(0x51),
            0x74 => Some(0x4f),
            0x75 => Some(0x52),
            0x7a => Some(0x4e),
            0x7d => Some(0x4b),
            _ => None,
        };
    }
    match code {
        0x1c => Some(4),
        0x32 => Some(5),
        0x21 => Some(6),
        0x23 => Some(7),
        0x24 => Some(8),
        0x2b => Some(9),
        0x34 => Some(10),
        0x33 => Some(11),
        0x43 => Some(12),
        0x3b => Some(13),
        0x42 => Some(14),
        0x4b => Some(15),
        0x3a => Some(16),
        0x31 => Some(17),
        0x44 => Some(18),
        0x4d => Some(19),
        0x15 => Some(20),
        0x2d => Some(21),
        0x1b => Some(22),
        0x2c => Some(23),
        0x3c => Some(24),
        0x2a => Some(25),
        0x1d => Some(26),
        0x22 => Some(27),
        0x35 => Some(28),
        0x1a => Some(29),
        0x16 => Some(30),
        0x1e => Some(31),
        0x26 => Some(32),
        0x25 => Some(33),
        0x2e => Some(34),
        0x36 => Some(35),
        0x3d => Some(36),
        0x3e => Some(37),
        0x46 => Some(38),
        0x45 => Some(39),
        0x5a => Some(40),
        0x76 => Some(41),
        0x66 => Some(42),
        0x0d => Some(43),
        0x29 => Some(44),
        0x4e => Some(45),
        0x55 => Some(46),
        0x54 => Some(47),
        0x5b => Some(48),
        0x5d => Some(49),
        0x4c => Some(51),
        0x52 => Some(52),
        0x0e => Some(53),
        0x41 => Some(54),
        0x49 => Some(55),
        0x4a => Some(56),
        0x58 => Some(57),
        0x05 => Some(58),
        0x06 => Some(59),
        0x04 => Some(60),
        0x0c => Some(61),
        0x03 => Some(62),
        0x0b => Some(63),
        0x83 => Some(64),
        0x0a => Some(65),
        0x01 => Some(66),
        0x09 => Some(67),
        0x78 => Some(68),
        0x07 => Some(69),
        0x14 => Some(0xe0),
        0x12 => Some(0xe1),
        0x11 => Some(0xe2),
        0x59 => Some(0xe5),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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
    fn malformed_and_unsupported_input_refuse() {
        let mut keyboard = KeyboardDecoder::new();
        assert_eq!(keyboard.accept(0), Err(Ps2Error::UnsupportedKeyboardCode));
        let mut pointer = PointerDecoder::new();
        assert_eq!(pointer.accept(0), Err(Ps2Error::MalformedPointerPacket));
    }
}

//! Portable keyboard transitions, independent of any device or platform Base.

use conduit_core::{semantic_digest, InfoDecodeError};

pub const KEY_EVENT_INFO_ID: &str = "input/key-event@1";
pub const KEY_EVENT_ENCODED_LEN: usize = 3;
pub const KEYBOARD_USAGE_MINIMUM: u8 = 0x04;
pub const KEYBOARD_USAGE_MAXIMUM: u8 = 0xa4;
pub const MODIFIER_USAGE_MINIMUM: u8 = 0xe0;
pub const MODIFIER_USAGE_MAXIMUM: u8 = 0xe7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEventConformanceVector {
    pub name: &'static str,
    pub encoded: [u8; KEY_EVENT_ENCODED_LEN],
}

/// Ordered values reusable by every exact keyboard implementation.
pub const KEY_EVENT_CONFORMANCE_VECTORS: [KeyEventConformanceVector; 8] = [
    vector("a-pressed", 0x04, 0, 0),
    vector("a-released", 0x04, 1, 0),
    vector("left-shift-pressed", 0xe1, 0, 0x02),
    vector("shift-a-pressed", 0x04, 0, 0x02),
    vector("shift-a-released", 0x04, 1, 0x02),
    vector("left-shift-released", 0xe1, 1, 0),
    vector("simultaneous-a-first", 0x04, 0, 0),
    vector("simultaneous-b-second", 0x05, 0, 0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum KeyTransition {
    Pressed = 0,
    Released = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const LEFT_CONTROL: Self = Self(1 << 0);
    pub const LEFT_SHIFT: Self = Self(1 << 1);
    pub const LEFT_ALT: Self = Self(1 << 2);
    pub const LEFT_GUI: Self = Self(1 << 3);
    pub const RIGHT_CONTROL: Self = Self(1 << 4);
    pub const RIGHT_SHIFT: Self = Self(1 << 5);
    pub const RIGHT_ALT: Self = Self(1 << 6);
    pub const RIGHT_GUI: Self = Self(1 << 7);

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains_usage(self, usage: u8) -> bool {
        if usage < MODIFIER_USAGE_MINIMUM || usage > MODIFIER_USAGE_MAXIMUM {
            return false;
        }
        self.0 & (1 << (usage - MODIFIER_USAGE_MINIMUM)) != 0
    }
}

/// One exact keyboard transition with the modifier state *after* the transition.
///
/// Usage numbers use the USB HID Keyboard/Keypad page as a host-neutral
/// vocabulary. That choice does not imply a USB device or transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyEvent {
    usage: u8,
    transition: KeyTransition,
    modifiers_after: KeyModifiers,
}

impl KeyEvent {
    pub fn new(
        usage: u8,
        transition: KeyTransition,
        modifiers_after: KeyModifiers,
    ) -> Result<Self, InfoDecodeError> {
        if !is_canonical_keyboard_usage(usage) {
            return Err(InfoDecodeError::ReservedValue {
                field: "keyboard-usage",
                actual: usage,
            });
        }
        if is_modifier_usage(usage)
            && modifiers_after.contains_usage(usage) != matches!(transition, KeyTransition::Pressed)
        {
            return Err(InfoDecodeError::InconsistentValue(
                "modifier-after-transition",
            ));
        }
        Ok(Self {
            usage,
            transition,
            modifiers_after,
        })
    }

    pub const fn usage(self) -> u8 {
        self.usage
    }

    pub const fn transition(self) -> KeyTransition {
        self.transition
    }

    pub const fn modifiers_after(self) -> KeyModifiers {
        self.modifiers_after
    }

    pub const fn encode(self) -> [u8; KEY_EVENT_ENCODED_LEN] {
        [
            self.usage,
            self.transition as u8,
            self.modifiers_after.bits(),
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        if encoded.len() != KEY_EVENT_ENCODED_LEN {
            return Err(InfoDecodeError::WrongLength {
                expected: KEY_EVENT_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        let transition = match encoded[1] {
            0 => KeyTransition::Pressed,
            1 => KeyTransition::Released,
            other => return Err(InfoDecodeError::NonCanonicalEnum(other)),
        };
        Self::new(encoded[0], transition, KeyModifiers::from_bits(encoded[2]))
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(KEY_EVENT_INFO_ID, &self.encode())
    }
}

pub const fn is_modifier_usage(usage: u8) -> bool {
    usage >= MODIFIER_USAGE_MINIMUM && usage <= MODIFIER_USAGE_MAXIMUM
}

pub const fn is_canonical_keyboard_usage(usage: u8) -> bool {
    (usage >= KEYBOARD_USAGE_MINIMUM && usage <= KEYBOARD_USAGE_MAXIMUM) || is_modifier_usage(usage)
}

const fn vector(
    name: &'static str,
    usage: u8,
    transition: u8,
    modifiers: u8,
) -> KeyEventConformanceVector {
    KeyEventConformanceVector {
        name,
        encoded: [usage, transition, modifiers],
    }
}

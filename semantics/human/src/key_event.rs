//! Portable keyboard transitions, independent of any device or platform Base.

use conduit_core::{
    semantic_digest, AdmissionUnit, DeliveryContract, DeliveryPressurePolicy, EvolutionSemantics,
    InfoDecodeError,
};

pub const KEY_EVENT_INFO_ID: &str = "input/key-event@1";
pub const KEY_EVENT_ENCODED_LEN: usize = 3;
pub const KEYBOARD_USAGE_MINIMUM: u8 = 0x04;
pub const KEYBOARD_USAGE_MAXIMUM: u8 = 0xa4;
pub const MODIFIER_USAGE_MINIMUM: u8 = 0xe0;
pub const MODIFIER_USAGE_MAXIMUM: u8 = 0xe7;
pub const KEY_EVENT_DELIVERY_CONTRACT: DeliveryContract = DeliveryContract::new(
    EvolutionSemantics::Occurrence,
    AdmissionUnit::Value,
    DeliveryPressurePolicy::PreserveOrder,
);

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

use crate::{KeyModifiers, KeyTransition, KeyTransitionCode};

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
            KeyTransitionCode::encode(self.transition)[0],
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
        let transition = KeyTransitionCode::decode(&encoded[1..2])
            .map_err(|_| InfoDecodeError::NonCanonicalEnum(encoded[1]))?;
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

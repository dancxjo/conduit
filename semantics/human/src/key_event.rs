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

use crate::{KeyEvent, KeyModifiers, KeyTransition, KeyTransitionForm};

/// One exact keyboard transition with the modifier state *after* the transition.
///
/// Usage numbers use the USB HID Keyboard/Keypad page as a host-neutral
/// vocabulary. That choice does not imply a USB device or transport.
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
        let contains = |modifier: KeyModifiers| modifiers_after.bits() & modifier.bits() != 0;
        Ok(Self::new_native(
            usage,
            transition,
            contains(KeyModifiers::LEFT_CONTROL),
            contains(KeyModifiers::LEFT_SHIFT),
            contains(KeyModifiers::LEFT_ALT),
            contains(KeyModifiers::LEFT_GUI),
            contains(KeyModifiers::RIGHT_CONTROL),
            contains(KeyModifiers::RIGHT_SHIFT),
            contains(KeyModifiers::RIGHT_ALT),
            contains(KeyModifiers::RIGHT_GUI),
        )
        .expect("explicit keyboard checks match generated contract"))
    }

    pub fn modifiers_after(&self) -> KeyModifiers {
        let fields = [
            (self.left_control_after(), KeyModifiers::LEFT_CONTROL),
            (self.left_shift_after(), KeyModifiers::LEFT_SHIFT),
            (self.left_alt_after(), KeyModifiers::LEFT_ALT),
            (self.left_gui_after(), KeyModifiers::LEFT_GUI),
            (self.right_control_after(), KeyModifiers::RIGHT_CONTROL),
            (self.right_shift_after(), KeyModifiers::RIGHT_SHIFT),
            (self.right_alt_after(), KeyModifiers::RIGHT_ALT),
            (self.right_gui_after(), KeyModifiers::RIGHT_GUI),
        ];
        KeyModifiers::from_bits(fields.into_iter().fold(0, |bits, (present, modifier)| {
            bits | if present { modifier.bits() } else { 0 }
        }))
    }

    pub fn encode(self) -> [u8; KEY_EVENT_ENCODED_LEN] {
        [
            self.usage(),
            KeyTransitionForm::encode(self.transition())[0],
            self.modifiers_after().bits(),
        ]
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        if encoded.len() != KEY_EVENT_ENCODED_LEN {
            return Err(InfoDecodeError::WrongLength {
                expected: KEY_EVENT_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        let transition = KeyTransitionForm::decode(&encoded[1..2])
            .map_err(|_| InfoDecodeError::NonCanonicalEnum(encoded[1]))?;
        Self::new(encoded[0], transition, KeyModifiers::from_bits(encoded[2]))
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(KEY_EVENT_INFO_ID, &self.encode())
    }
}

impl PartialOrd for KeyEvent {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyEvent {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.usage(), self.transition(), self.modifiers_after()).cmp(&(
            other.usage(),
            other.transition(),
            other.modifiers_after(),
        ))
    }
}

impl core::hash::Hash for KeyEvent {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.usage().hash(state);
        self.transition().hash(state);
        self.modifiers_after().hash(state);
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

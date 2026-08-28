//! Portable modifier-chord meaning, separate from text and product actions.

use conduit_core::{semantic_digest, InfoDecodeError};

use crate::{KeyEvent, KeyModifiers, KeyTransition};

pub const CHORD_INFO_ID: &str = "input/chord@1";
pub const CHORD_ENCODED_LEN: usize = 4;
pub const CORE_CHORD_MAP: &str = "conduit-core";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ChordPhase {
    Triggered = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum CoreChordId {
    CancelOrEscape = 1,
    ClearOrRefresh = 2,
    RepeatOrReplan = 3,
    Palette = 4,
    Inspect = 5,
    Plan = 6,
    Command = 7,
    Activate = 8,
}

impl CoreChordId {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::CancelOrEscape => "chord/cancel-or-escape",
            Self::ClearOrRefresh => "chord/clear-or-refresh",
            Self::RepeatOrReplan => "chord/repeat-or-replan",
            Self::Palette => "chord/palette",
            Self::Inspect => "chord/inspect",
            Self::Plan => "chord/plan",
            Self::Command => "chord/command",
            Self::Activate => "chord/activate",
        }
    }

    const fn decode(value: u8) -> Option<Self> {
        Some(match value {
            1 => Self::CancelOrEscape,
            2 => Self::ClearOrRefresh,
            3 => Self::RepeatOrReplan,
            4 => Self::Palette,
            5 => Self::Inspect,
            6 => Self::Plan,
            7 => Self::Command,
            8 => Self::Activate,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChordInfo {
    modifiers: KeyModifiers,
    usage: u8,
    phase: ChordPhase,
    chord_id: CoreChordId,
}

impl ChordInfo {
    pub fn from_key_event(event: KeyEvent) -> Option<Self> {
        if event.transition() != KeyTransition::Pressed {
            return None;
        }
        let chord_id = core_chord_id(event.modifiers_after(), event.usage())?;
        Some(Self {
            modifiers: event.modifiers_after(),
            usage: event.usage(),
            phase: ChordPhase::Triggered,
            chord_id,
        })
    }

    pub const fn modifiers(self) -> KeyModifiers {
        self.modifiers
    }

    pub const fn usage(self) -> u8 {
        self.usage
    }

    pub const fn phase(self) -> ChordPhase {
        self.phase
    }

    pub const fn chord_id(self) -> CoreChordId {
        self.chord_id
    }

    pub const fn encode(self) -> [u8; CHORD_ENCODED_LEN] {
        [
            self.modifiers.bits(),
            self.usage,
            self.phase as u8,
            self.chord_id as u8,
        ]
    }

    pub fn semantic_digest(self) -> [u8; 32] {
        semantic_digest(CHORD_INFO_ID, &self.encode())
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, InfoDecodeError> {
        if encoded.len() != CHORD_ENCODED_LEN {
            return Err(InfoDecodeError::WrongLength {
                expected: CHORD_ENCODED_LEN,
                actual: encoded.len(),
            });
        }
        if encoded[2] != ChordPhase::Triggered as u8 {
            return Err(InfoDecodeError::NonCanonicalEnum(encoded[2]));
        }
        let modifiers = KeyModifiers::from_bits(encoded[0]);
        let Some(chord_id) = CoreChordId::decode(encoded[3]) else {
            return Err(InfoDecodeError::NonCanonicalEnum(encoded[3]));
        };
        if core_chord_id(modifiers, encoded[1]) != Some(chord_id) {
            return Err(InfoDecodeError::InconsistentValue("canonical-chord-id"));
        }
        Ok(Self {
            modifiers,
            usage: encoded[1],
            phase: ChordPhase::Triggered,
            chord_id,
        })
    }
}

/// The exact first `conduit-core` vocabulary. Unknown combinations emit no
/// mapped chord; their structural key events remain available upstream.
pub const fn core_chord_id(modifiers: KeyModifiers, usage: u8) -> Option<CoreChordId> {
    let bits = modifiers.bits();
    if bits & (KeyModifiers::RIGHT_ALT.bits() | KeyModifiers::RIGHT_GUI.bits()) != 0 {
        return None;
    }
    let control = bits & (KeyModifiers::LEFT_CONTROL.bits() | KeyModifiers::RIGHT_CONTROL.bits());
    let left_alt = bits & KeyModifiers::LEFT_ALT.bits();
    let left_meta = bits & KeyModifiers::LEFT_GUI.bits();
    let shift = bits & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits());
    if shift != 0 {
        return None;
    }
    match (control != 0, left_alt != 0, left_meta != 0, usage) {
        (true, false, false, 0x0a) => Some(CoreChordId::CancelOrEscape), // G
        (true, false, false, 0x0f) => Some(CoreChordId::ClearOrRefresh), // L
        (true, false, false, 0x15) => Some(CoreChordId::RepeatOrReplan), // R
        (false, true, false, 0x13) => Some(CoreChordId::Palette),        // P
        (false, true, false, 0x0c) => Some(CoreChordId::Inspect),        // I
        (false, false, true, 0x13) => Some(CoreChordId::Plan),           // P
        (false, false, true, 0x2c) => Some(CoreChordId::Command),        // Space
        (false, false, true, 0x28) => Some(CoreChordId::Activate),       // Enter
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pressed(usage: u8, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(usage, KeyTransition::Pressed, modifiers).unwrap()
    }

    #[test]
    fn default_map_is_exact_and_right_modifiers_are_reserved() {
        let ctrl_g = ChordInfo::from_key_event(pressed(0x0a, KeyModifiers::LEFT_CONTROL)).unwrap();
        assert_eq!(ctrl_g.chord_id(), CoreChordId::CancelOrEscape);
        assert_eq!(ChordInfo::decode(&ctrl_g.encode()), Ok(ctrl_g));
        assert_eq!(
            ChordInfo::from_key_event(pressed(0x13, KeyModifiers::LEFT_ALT))
                .unwrap()
                .chord_id(),
            CoreChordId::Palette
        );
        assert_eq!(
            ChordInfo::from_key_event(pressed(0x13, KeyModifiers::LEFT_GUI))
                .unwrap()
                .chord_id(),
            CoreChordId::Plan
        );
        assert!(ChordInfo::from_key_event(pressed(0x08, KeyModifiers::RIGHT_ALT)).is_none());
        assert!(ChordInfo::from_key_event(pressed(0x13, KeyModifiers::RIGHT_GUI)).is_none());
        assert!(ChordInfo::from_key_event(pressed(0x04, KeyModifiers::LEFT_CONTROL)).is_none());
    }
}

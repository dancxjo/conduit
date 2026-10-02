//! Portable modifier-chord meaning, separate from text and product actions.

use conduit_core::{semantic_digest, InfoDecodeError};

use crate::{
    ChordInfo, ChordPhase, ChordPhaseForm, ControlChordModifier, CoreChordId, CoreChordIdForm,
    KeyEvent, KeyModifiers, KeyTransition,
};

pub const CHORD_INFO_ID: &str = "input/chord@1";
pub const CHORD_ENCODED_LEN: usize = 4;
pub const CORE_CHORD_MAP: &str = "conduit-core";

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
}

impl ChordInfo {
    pub fn from_key_event(event: KeyEvent) -> Option<Self> {
        if event.transition() != KeyTransition::Pressed {
            return None;
        }
        Self::from_parts(
            event.modifiers_after(),
            event.usage(),
            core_chord_id(event.modifiers_after(), event.usage())?,
        )
    }

    pub const fn modifiers(self) -> KeyModifiers {
        match self {
            Self::CancelOrEscape(value)
            | Self::ClearOrRefresh(value)
            | Self::RepeatOrReplan(value) => value.modifiers(),
            Self::Palette | Self::Inspect => KeyModifiers::LEFT_ALT,
            Self::Plan | Self::Command | Self::Activate => KeyModifiers::LEFT_GUI,
        }
    }

    pub const fn usage(self) -> u8 {
        match self {
            Self::CancelOrEscape(_) => 0x0a,
            Self::ClearOrRefresh(_) => 0x0f,
            Self::RepeatOrReplan(_) => 0x15,
            Self::Palette | Self::Plan => 0x13,
            Self::Inspect => 0x0c,
            Self::Command => 0x2c,
            Self::Activate => 0x28,
        }
    }

    pub const fn phase(self) -> ChordPhase {
        ChordPhase::Triggered
    }

    pub const fn chord_id(self) -> CoreChordId {
        match self {
            Self::CancelOrEscape(_) => CoreChordId::CancelOrEscape,
            Self::ClearOrRefresh(_) => CoreChordId::ClearOrRefresh,
            Self::RepeatOrReplan(_) => CoreChordId::RepeatOrReplan,
            Self::Palette => CoreChordId::Palette,
            Self::Inspect => CoreChordId::Inspect,
            Self::Plan => CoreChordId::Plan,
            Self::Command => CoreChordId::Command,
            Self::Activate => CoreChordId::Activate,
        }
    }

    pub const fn encode(self) -> [u8; CHORD_ENCODED_LEN] {
        [
            self.modifiers().bits(),
            self.usage(),
            ChordPhaseForm::encode(self.phase())[0],
            CoreChordIdForm::encode(self.chord_id())[0],
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
        let phase = ChordPhaseForm::decode(&encoded[2..3])
            .map_err(|_| InfoDecodeError::NonCanonicalEnum(encoded[2]))?;
        let modifiers = KeyModifiers::from_bits(encoded[0]);
        let chord_id = CoreChordIdForm::decode(&encoded[3..4])
            .map_err(|_| InfoDecodeError::NonCanonicalEnum(encoded[3]))?;
        if core_chord_id(modifiers, encoded[1]) != Some(chord_id) {
            return Err(InfoDecodeError::InconsistentValue("canonical-chord-id"));
        }
        if phase != ChordPhase::Triggered {
            return Err(InfoDecodeError::InconsistentValue("canonical-chord-phase"));
        }
        Self::from_parts(modifiers, encoded[1], chord_id)
            .ok_or(InfoDecodeError::InconsistentValue("canonical-chord-id"))
    }

    fn from_parts(modifiers: KeyModifiers, usage: u8, chord_id: CoreChordId) -> Option<Self> {
        match chord_id {
            CoreChordId::CancelOrEscape if usage == 0x0a => Some(Self::CancelOrEscape(
                ControlChordModifier::from_modifiers(modifiers)?,
            )),
            CoreChordId::ClearOrRefresh if usage == 0x0f => Some(Self::ClearOrRefresh(
                ControlChordModifier::from_modifiers(modifiers)?,
            )),
            CoreChordId::RepeatOrReplan if usage == 0x15 => Some(Self::RepeatOrReplan(
                ControlChordModifier::from_modifiers(modifiers)?,
            )),
            CoreChordId::Palette
                if usage == 0x13 && modifiers.bits() == KeyModifiers::LEFT_ALT.bits() =>
            {
                Some(Self::Palette)
            }
            CoreChordId::Inspect
                if usage == 0x0c && modifiers.bits() == KeyModifiers::LEFT_ALT.bits() =>
            {
                Some(Self::Inspect)
            }
            CoreChordId::Plan
                if usage == 0x13 && modifiers.bits() == KeyModifiers::LEFT_GUI.bits() =>
            {
                Some(Self::Plan)
            }
            CoreChordId::Command
                if usage == 0x2c && modifiers.bits() == KeyModifiers::LEFT_GUI.bits() =>
            {
                Some(Self::Command)
            }
            CoreChordId::Activate
                if usage == 0x28 && modifiers.bits() == KeyModifiers::LEFT_GUI.bits() =>
            {
                Some(Self::Activate)
            }
            _ => None,
        }
    }
}

impl PartialOrd for ChordInfo {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ChordInfo {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl core::hash::Hash for ChordInfo {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        core::hash::Hash::hash(&self.encode(), state);
    }
}

impl ControlChordModifier {
    const fn from_modifiers(modifiers: KeyModifiers) -> Option<Self> {
        match modifiers.bits() {
            value if value == KeyModifiers::LEFT_CONTROL.bits() => Some(Self::Left),
            value if value == KeyModifiers::RIGHT_CONTROL.bits() => Some(Self::Right),
            value
                if value
                    == KeyModifiers::LEFT_CONTROL.bits() | KeyModifiers::RIGHT_CONTROL.bits() =>
            {
                Some(Self::Both)
            }
            _ => None,
        }
    }

    const fn modifiers(self) -> KeyModifiers {
        match self {
            Self::Left => KeyModifiers::LEFT_CONTROL,
            Self::Right => KeyModifiers::RIGHT_CONTROL,
            Self::Both => KeyModifiers::from_bits(
                KeyModifiers::LEFT_CONTROL.bits() | KeyModifiers::RIGHT_CONTROL.bits(),
            ),
        }
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

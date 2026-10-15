//! Authored layout identities prepared before Play; selection never reparses layouts.
use super::*;

pub fn choices_type() -> StructuredInfoType {
    StructuredInfoType::from_canonical_bytes(KEYMAP_CHOICES_TYPE_BYTES)
        .expect("checked authored choices Type")
}
pub fn default_choices_configuration() -> StructuredConfigurationValue {
    StructuredConfigurationValue::new(
        choices_type()
            .profile()
            .expect("finite choices")
            .value_kind()
            .clone(),
        DEFAULT_KEYMAP_CHOICES_BYTES.to_vec(),
    )
    .expect("checked choices")
}

pub struct PreparedKeymapChoices {
    initial: PreparedKeymap,
    choices: Vec<(String, PreparedKeymap)>,
    selected: Option<usize>,
}
impl PreparedKeymapChoices {
    pub fn new(
        layout: &StructuredConfigurationValue,
        choices: &StructuredConfigurationValue,
    ) -> Result<Self, String> {
        if choices.profile()
            != choices_type()
                .profile()
                .map_err(|_| "invalid choices type")?
                .value_kind()
        {
            return Err("keymap choices have the wrong exact type".into());
        }
        let value = StructuredInfoValue::from_canonical_bytes(choices.canonical_value())
            .map_err(|_| "invalid keymap choices")?;
        let mut prepared = Vec::new();
        for choice in collection(&value)? {
            let identity = text(field(choice, "identity")?)?;
            if identity.is_empty()
                || identity.len() > 32
                || prepared.iter().any(|(id, _)| id == identity)
            {
                return Err(
                    "keymap choice identity is empty, duplicated, or exceeds 32 bytes".into(),
                );
            }
            let layout = field(choice, "layout")?;
            let configuration = StructuredConfigurationValue::new(
                layout
                    .value_type()
                    .profile()
                    .map_err(|_| "invalid choice layout type")?
                    .value_kind()
                    .clone(),
                layout
                    .canonical_bytes()
                    .map_err(|_| "invalid choice layout")?,
            )
            .ok_or("choice layout exceeds configuration bound")?;
            prepared.push((identity.to_string(), PreparedKeymap::new(&configuration)?));
        }
        Ok(Self {
            initial: PreparedKeymap::new(layout)?,
            choices: prepared,
            selected: None,
        })
    }
    pub fn select(&mut self, identity: &[u8]) -> Result<(), &'static str> {
        let index = self
            .choices
            .iter()
            .position(|(id, _)| id.as_bytes() == identity)
            .ok_or("unknown keymap choice identity")?;
        if self.selected != Some(index) {
            // Locks belong to the live keyboard gear, not to an inactive layout.
            let active = match self.selected {
                Some(index) => &self.choices[index].1,
                None => &self.initial,
            };
            let (caps_lock, num_lock) = (active.caps_lock, active.num_lock);
            self.reset();
            self.selected = Some(index);
            let target = &mut self.choices[index].1;
            target.reset();
            target.caps_lock = caps_lock;
            target.num_lock = num_lock;
        }
        Ok(())
    }
    pub fn apply(&mut self, event: KeyEvent) -> Result<Option<KeymapText>, KeymapRefusal> {
        match self.selected {
            Some(index) => self.choices[index].1.apply(event),
            None => self.initial.apply(event),
        }
    }
    pub fn reset(&mut self) {
        match self.selected {
            Some(index) => self.choices[index].1.reset(),
            None => self.initial.reset(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KeyModifiers, KeyTransition};
    fn press(usage: u8, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(usage, KeyTransition::Pressed, modifiers).unwrap()
    }
    #[test]
    fn selecting_prepared_identities_switches_layout_and_clears_pending_dead_state() {
        let mut map = PreparedKeymapChoices::new(
            &default_keymap_configuration(),
            &default_choices_configuration(),
        )
        .unwrap();
        assert!(map
            .apply(press(52, KeyModifiers::RIGHT_ALT))
            .unwrap()
            .is_none());
        assert!(map.select(b"unregistered").is_err());
        assert_eq!(
            map.apply(press(8, KeyModifiers::NONE))
                .unwrap()
                .unwrap()
                .as_slice(),
            "é".as_bytes()
        );
        assert!(map
            .apply(press(52, KeyModifiers::RIGHT_ALT))
            .unwrap()
            .is_none());
        map.select(b"us").unwrap();
        assert_eq!(
            map.apply(press(8, KeyModifiers::NONE))
                .unwrap()
                .unwrap()
                .as_slice(),
            b"e"
        );
        map.select(b"german").unwrap();
        assert_eq!(
            map.apply(press(28, KeyModifiers::NONE))
                .unwrap()
                .unwrap()
                .as_slice(),
            b"z"
        );
        map.select(b"french").unwrap();
        assert_eq!(
            map.apply(press(4, KeyModifiers::NONE))
                .unwrap()
                .unwrap()
                .as_slice(),
            b"q"
        );
    }
    #[test]
    fn selecting_current_identity_preserves_in_progress_composition() {
        let mut map = PreparedKeymapChoices::new(
            &default_keymap_configuration(),
            &default_choices_configuration(),
        )
        .unwrap();
        map.select(b"us-intl").unwrap();
        assert!(map
            .apply(press(52, KeyModifiers::RIGHT_ALT))
            .unwrap()
            .is_none());
        map.select(b"us-intl").unwrap();
        assert_eq!(
            map.apply(press(8, KeyModifiers::NONE))
                .unwrap()
                .unwrap()
                .as_slice(),
            "é".as_bytes()
        );
    }
    #[test]
    fn changing_layout_preserves_live_caps_and_num_lock_including_toggled_off_state() {
        let mut map = PreparedKeymapChoices::new(
            &default_keymap_configuration(),
            &default_choices_configuration(),
        )
        .unwrap();
        // Initial custom/default state transfers to the first selected preset too.
        assert!(map.apply(press(57, KeyModifiers::NONE)).unwrap().is_none());
        assert!(map.apply(press(83, KeyModifiers::NONE)).unwrap().is_none());
        for (identity, usage, expected) in [
            (b"us".as_slice(), 4, "A"),
            (b"german".as_slice(), 45, "ẞ"),
            (b"french".as_slice(), 31, "É"),
        ] {
            map.select(identity).unwrap();
            assert_eq!(
                map.apply(press(usage, KeyModifiers::NONE))
                    .unwrap()
                    .unwrap()
                    .as_slice(),
                expected.as_bytes()
            );
            assert_eq!(
                map.apply(press(89, KeyModifiers::NONE))
                    .unwrap()
                    .unwrap()
                    .as_slice(),
                b"1"
            );
        }
        assert!(map.apply(press(57, KeyModifiers::NONE)).unwrap().is_none());
        assert!(map.apply(press(83, KeyModifiers::NONE)).unwrap().is_none());
        for (identity, usage, expected) in [
            (b"german".as_slice(), 45, "ß"),
            (b"us".as_slice(), 4, "a"),
            (b"french".as_slice(), 31, "é"),
        ] {
            map.select(identity).unwrap();
            assert_eq!(
                map.apply(press(usage, KeyModifiers::NONE))
                    .unwrap()
                    .unwrap()
                    .as_slice(),
                expected.as_bytes()
            );
            assert!(map.apply(press(89, KeyModifiers::NONE)).unwrap().is_none());
        }
    }
}

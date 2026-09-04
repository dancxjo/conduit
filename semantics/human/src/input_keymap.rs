//! Finite, host-neutral `conduit-intl` keyboard text semantics.

use crate::{KeyEvent, KeyModifiers, KeyTransition};

pub const CONDUIT_INTL_LAYOUT: &str = "conduit-intl";
pub const KEYMAP_MAXIMUM_HEX_DIGITS: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextFragment {
    bytes: [u8; 4],
    len: u8,
}

impl TextFragment {
    pub fn from_char(value: char) -> Self {
        let mut bytes = [0; 4];
        let len = value.encode_utf8(&mut bytes).len() as u8;
        Self { bytes, len }
    }

    pub const fn bytes(&self) -> &[u8; 4] {
        &self.bytes
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapRefusal {
    UnknownComposeSequence,
    EmptyUnicodeEntry,
    UnicodeEntryOverflow,
    InvalidUnicodeScalar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapDisposition {
    NoText,
    Text(TextFragment),
    Cancelled,
    Refused(KeymapRefusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Direct,
    ComposeFirst,
    ComposeSecond(char),
    Unicode { value: u32, digits: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitIntlKeymap {
    mode: Mode,
    right_meta_tap: bool,
}

impl Default for ConduitIntlKeymap {
    fn default() -> Self {
        Self::new()
    }
}

impl ConduitIntlKeymap {
    pub const fn new() -> Self {
        Self {
            mode: Mode::Direct,
            right_meta_tap: false,
        }
    }

    pub const fn reset(&mut self) {
        self.mode = Mode::Direct;
        self.right_meta_tap = false;
    }

    pub fn apply(&mut self, event: KeyEvent) -> KeymapDisposition {
        if event.usage() == 0xe7 {
            match event.transition() {
                KeyTransition::Pressed if matches!(self.mode, Mode::Direct) => {
                    self.right_meta_tap = true;
                }
                KeyTransition::Released if self.right_meta_tap => {
                    self.right_meta_tap = false;
                    self.mode = Mode::ComposeFirst;
                }
                KeyTransition::Released => self.right_meta_tap = false,
                _ => {}
            }
            return KeymapDisposition::NoText;
        }
        if event.transition() == KeyTransition::Released {
            return KeymapDisposition::NoText;
        }
        if event.usage() == 0x29 && !matches!(self.mode, Mode::Direct) {
            self.reset();
            return KeymapDisposition::Cancelled;
        }

        if event.modifiers_after().bits() & KeyModifiers::RIGHT_GUI.bits() != 0 {
            self.right_meta_tap = false;
            if matches!(self.mode, Mode::Direct) && event.usage() == 0x18 {
                self.mode = Mode::Unicode {
                    value: 0,
                    digits: 0,
                };
            }
            return KeymapDisposition::NoText;
        }

        // Keyboard fallback: AltGr+Space starts Compose; AltGr+Shift+Space
        // starts direct Unicode entry on hardware without Right Meta.
        if matches!(self.mode, Mode::Direct)
            && event.modifiers_after().bits() & KeyModifiers::RIGHT_ALT.bits() != 0
            && event.usage() == 0x2c
        {
            self.mode = if shifted(event.modifiers_after()) {
                Mode::Unicode {
                    value: 0,
                    digits: 0,
                }
            } else {
                Mode::ComposeFirst
            };
            return KeymapDisposition::NoText;
        }

        match self.mode {
            Mode::Direct => self.direct(event),
            Mode::ComposeFirst => {
                let Some(first) = qwerty_ascii(event.usage(), shifted(event.modifiers_after()))
                else {
                    self.reset();
                    return KeymapDisposition::Refused(KeymapRefusal::UnknownComposeSequence);
                };
                self.mode = Mode::ComposeSecond(first);
                KeymapDisposition::NoText
            }
            Mode::ComposeSecond(first) => {
                let second = qwerty_ascii(event.usage(), shifted(event.modifiers_after()));
                self.reset();
                match second.and_then(|second| compose(first, second)) {
                    Some(value) => KeymapDisposition::Text(TextFragment::from_char(value)),
                    None => KeymapDisposition::Refused(KeymapRefusal::UnknownComposeSequence),
                }
            }
            Mode::Unicode { value, digits } => self.unicode(event, value, digits),
        }
    }

    fn direct(&mut self, event: KeyEvent) -> KeymapDisposition {
        let modifiers = event.modifiers_after().bits();
        if modifiers
            & (KeyModifiers::LEFT_CONTROL.bits()
                | KeyModifiers::RIGHT_CONTROL.bits()
                | KeyModifiers::LEFT_ALT.bits()
                | KeyModifiers::LEFT_GUI.bits())
            != 0
        {
            return KeymapDisposition::NoText;
        }
        if modifiers & KeyModifiers::RIGHT_ALT.bits() != 0 {
            return altgr(event.usage(), shifted(event.modifiers_after()))
                .map(TextFragment::from_char)
                .map(KeymapDisposition::Text)
                .unwrap_or(KeymapDisposition::NoText);
        }
        qwerty_ascii(event.usage(), shifted(event.modifiers_after()))
            .map(TextFragment::from_char)
            .map(KeymapDisposition::Text)
            .unwrap_or(KeymapDisposition::NoText)
    }

    fn unicode(&mut self, event: KeyEvent, value: u32, digits: u8) -> KeymapDisposition {
        if event.usage() == 0x28 {
            self.reset();
            if digits == 0 {
                return KeymapDisposition::Refused(KeymapRefusal::EmptyUnicodeEntry);
            }
            return match char::from_u32(value) {
                Some(value) => KeymapDisposition::Text(TextFragment::from_char(value)),
                None => KeymapDisposition::Refused(KeymapRefusal::InvalidUnicodeScalar),
            };
        }
        let Some(digit) = hex_digit(event.usage(), shifted(event.modifiers_after())) else {
            self.reset();
            return KeymapDisposition::Refused(KeymapRefusal::InvalidUnicodeScalar);
        };
        if digits == KEYMAP_MAXIMUM_HEX_DIGITS {
            self.reset();
            return KeymapDisposition::Refused(KeymapRefusal::UnicodeEntryOverflow);
        }
        let next = value.saturating_mul(16).saturating_add(u32::from(digit));
        if next > 0x10ffff {
            self.reset();
            return KeymapDisposition::Refused(KeymapRefusal::InvalidUnicodeScalar);
        }
        self.mode = Mode::Unicode {
            value: next,
            digits: digits + 1,
        };
        KeymapDisposition::NoText
    }
}

const fn shifted(modifiers: KeyModifiers) -> bool {
    modifiers.bits() & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits()) != 0
}

fn qwerty_ascii(usage: u8, shift: bool) -> Option<char> {
    if (0x04..=0x1d).contains(&usage) {
        let base = b'a' + (usage - 0x04);
        return Some(if shift {
            (base - b'a' + b'A') as char
        } else {
            base as char
        });
    }
    Some(match (usage, shift) {
        (0x1e, false) => '1',
        (0x1e, true) => '!',
        (0x1f, false) => '2',
        (0x1f, true) => '@',
        (0x20, false) => '3',
        (0x20, true) => '#',
        (0x21, false) => '4',
        (0x21, true) => '$',
        (0x22, false) => '5',
        (0x22, true) => '%',
        (0x23, false) => '6',
        (0x23, true) => '^',
        (0x24, false) => '7',
        (0x24, true) => '&',
        (0x25, false) => '8',
        (0x25, true) => '*',
        (0x26, false) => '9',
        (0x26, true) => '(',
        (0x27, false) => '0',
        (0x27, true) => ')',
        (0x28, _) => '\n',
        (0x2c, _) => ' ',
        (0x2d, false) => '-',
        (0x2d, true) => '_',
        (0x2e, false) => '=',
        (0x2e, true) => '+',
        (0x2f, false) => '[',
        (0x2f, true) => '{',
        (0x30, false) => ']',
        (0x30, true) => '}',
        (0x31, false) => '\\',
        (0x31, true) => '|',
        (0x33, false) => ';',
        (0x33, true) => ':',
        (0x34, false) => '\'',
        (0x34, true) => '"',
        (0x35, false) => '`',
        (0x35, true) => '~',
        (0x36, false) => ',',
        (0x36, true) => '<',
        (0x37, false) => '.',
        (0x37, true) => '>',
        (0x38, false) => '/',
        (0x38, true) => '?',
        _ => return None,
    })
}

fn hex_digit(usage: u8, shift: bool) -> Option<u8> {
    let value = qwerty_ascii(usage, shift)?.to_ascii_lowercase();
    value.to_digit(16).map(|digit| digit as u8)
}

const fn altgr(usage: u8, shift: bool) -> Option<char> {
    match (usage, shift) {
        (0x04, false) => Some('æ'),
        (0x12, false) => Some('ø'),
        (0x16, false) => Some('ß'),
        (0x07, false) => Some('ð'),
        (0x13, false) => Some('þ'),
        (0x11, false) => Some('ñ'),
        (0x08, false) => Some('€'),
        (0x0f, false) => Some('£'),
        (0x1c, false) => Some('¥'),
        (0x06, false) => Some('©'),
        (0x15, false) => Some('®'),
        (0x17, false) => Some('™'),
        (0x1e, true) => Some('¡'),
        (0x38, true) => Some('¿'),
        (0x2d, false) => Some('–'),
        (0x2d, true) => Some('—'),
        _ => None,
    }
}

fn compose(first: char, second: char) -> Option<char> {
    let uppercase = second.is_ascii_uppercase();
    let lower = second.to_ascii_lowercase();
    Some(match (first, lower, uppercase) {
        ('\'', 'e', false) => 'é',
        ('\'', 'e', true) => 'É',
        ('`', 'e', false) => 'è',
        ('`', 'e', true) => 'È',
        ('^', 'e', false) => 'ê',
        ('^', 'e', true) => 'Ê',
        ('"', 'e', false) => 'ë',
        ('"', 'e', true) => 'Ë',
        ('~', 'n', false) => 'ñ',
        ('~', 'n', true) => 'Ñ',
        (',', 'c', false) => 'ç',
        (',', 'c', true) => 'Ç',
        ('o', 'a', false) | ('O', 'a', false) => 'å',
        ('o', 'a', true) | ('O', 'a', true) => 'Å',
        ('/', 'o', false) => 'ø',
        ('/', 'o', true) => 'Ø',
        _ => return None,
    })
}

#[cfg(test)]
mod tests;

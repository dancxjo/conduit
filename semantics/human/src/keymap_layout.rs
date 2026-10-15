//! Prepared interpretation of authored, finite Conduitese keyboard layers.
//!
//! Tables are immutable during Play; at most four pending dead keys are retained.
use crate::{KeyEvent, KeyModifiers, KeyTransition, KeymapRefusal};
use alloc::{
    borrow::Cow,
    string::{String, ToString},
    vec::Vec,
};
use conduit_core::{
    StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape as Shape,
};

include!(concat!(env!("OUT_DIR"), "/keymap_layouts.rs"));
pub const KEYMAP_LAYOUT_SOURCE: &str = include_str!("../keymap-layouts.conduit");
pub const KEYMAP_MAXIMUM_TEXT_BYTES: usize = 48;

pub fn default_keymap_value() -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(DEFAULT_KEYMAP_BYTES)
        .expect("checked authored default keymap")
}
pub fn keymap_layout_type() -> StructuredInfoType {
    StructuredInfoType::from_canonical_bytes(KEYMAP_LAYOUT_TYPE_BYTES)
        .expect("checked authored keymap Type")
}
pub fn default_keymap_configuration() -> StructuredConfigurationValue {
    StructuredConfigurationValue::new(
        keymap_layout_type()
            .profile()
            .expect("finite layout")
            .value_kind()
            .clone(),
        DEFAULT_KEYMAP_BYTES.to_vec(),
    )
    .expect("checked authored keymap")
}

#[derive(Clone, Copy)]
struct Text {
    bytes: [u8; 8],
    len: u8,
}
impl Text {
    fn new(value: &str) -> Result<Self, String> {
        if value.len() > 8 {
            return Err("keymap text exceeds eight UTF-8 bytes".into());
        }
        let mut bytes = [0; 8];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self {
            bytes,
            len: value.len() as u8,
        })
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.len)]).expect("prepared UTF-8")
    }
}
#[derive(Clone, Copy)]
enum Action {
    None,
    CapsLock,
    CapsText { ordinary: Text, locked: Text },
    NumLock,
    Keypad(Text),
    Text(Text),
    Dead { spacing: Text, combining: char },
}
#[derive(Clone)]
struct Binding {
    usage: u8,
    shift: bool,
    altgr: bool,
    action: Action,
}
#[derive(Clone)]
struct Composition {
    combining: char,
    base: char,
    result: char,
}

pub struct KeymapText {
    bytes: [u8; KEYMAP_MAXIMUM_TEXT_BYTES],
    len: usize,
}
impl KeymapText {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    fn empty() -> Self {
        Self {
            bytes: [0; KEYMAP_MAXIMUM_TEXT_BYTES],
            len: 0,
        }
    }
    fn push(&mut self, text: &str) {
        let end = self.len + text.len();
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
    }
    fn character(&mut self, value: char) {
        let mut bytes = [0; 4];
        self.push(value.encode_utf8(&mut bytes));
    }
}

/// Finite source layout lowered once before Play. Applying keys allocates nothing.
pub struct PreparedKeymap {
    bindings: Cow<'static, [Binding]>,
    compositions: Cow<'static, [Composition]>,
    pending: [Option<(Text, char)>; 4],
    pending_len: usize,
    caps_lock: bool,
    num_lock: bool,
}
impl Default for PreparedKeymap {
    fn default() -> Self {
        Self::default_layout()
    }
}
impl PreparedKeymap {
    /// Source-checked default with no allocator dependency, including during preparation.
    pub const fn default_layout() -> Self {
        Self {
            bindings: Cow::Borrowed(DEFAULT_KEYMAP_BINDINGS),
            compositions: Cow::Borrowed(DEFAULT_KEYMAP_COMPOSITIONS),
            pending: [None; 4],
            pending_len: 0,
            caps_lock: false,
            num_lock: false,
        }
    }

    pub fn new(configuration: &StructuredConfigurationValue) -> Result<Self, String> {
        if let Some((_, bindings, compositions)) = KEYMAP_PREPARED_LAYOUTS
            .iter()
            .find(|(bytes, _, _)| *bytes == configuration.canonical_value())
        {
            return Ok(Self {
                bindings: Cow::Borrowed(bindings),
                compositions: Cow::Borrowed(compositions),
                pending: [None; 4],
                pending_len: 0,
                caps_lock: false,
                num_lock: false,
            });
        }

        if configuration.profile()
            != keymap_layout_type()
                .profile()
                .map_err(|_| "invalid layout type")?
                .value_kind()
        {
            return Err("keymap layout has the wrong exact type".into());
        }
        let value = StructuredInfoValue::from_canonical_bytes(configuration.canonical_value())
            .map_err(|_| "invalid keymap layout")?;
        let mut bindings = Vec::new();
        for layer in collection(field(&value, "layers")?)? {
            for binding in collection(layer)? {
                let usage = integer(field(binding, "usage")?)?;
                if !(4..=164).contains(&usage) && !(224..=231).contains(&usage) {
                    return Err("keymap usage is outside portable keyboard vocabulary".into());
                }
                let shift = boolean(field(binding, "shift")?)?;
                let altgr = boolean(field(binding, "altgr")?)?;
                let action = match field(binding, "action")?.shape() {
                    Shape::Variant { tag, payload } => match tag {
                        "none" => Action::None,
                        "caps_lock" => Action::CapsLock,
                        "caps_text" => Action::CapsText {
                            ordinary: Text::new(text(field(payload, "ordinary")?)?)?,
                            locked: Text::new(text(field(payload, "locked")?)?)?,
                        },
                        "num_lock" => Action::NumLock,
                        "keypad" => Action::Keypad(Text::new(text(payload)?)?),
                        "text" => Action::Text(Text::new(text(payload)?)?),
                        "dead" => {
                            let spacing = Text::new(text(field(payload, "spacing")?)?)?;
                            if spacing.len == 0 || spacing.len > 4 {
                                return Err("dead key requires one spacing scalar".into());
                            }
                            if spacing.as_str().chars().count() != 1 {
                                return Err("dead key spacing must be one scalar".into());
                            }
                            Action::Dead {
                                spacing,
                                combining: scalar(field(payload, "combining")?)?,
                            }
                        }
                        _ => return Err("unknown keymap action".into()),
                    },
                    _ => return Err("invalid keymap action".into()),
                };
                bindings.push(Binding {
                    usage: usage as u8,
                    shift,
                    altgr,
                    action,
                });
            }
        }
        let mut compositions = Vec::new();
        for composition in collection(field(&value, "compositions")?)? {
            compositions.push(Composition {
                combining: scalar(field(composition, "combining")?)?,
                base: scalar(field(composition, "base")?)?,
                result: scalar(field(composition, "result")?)?,
            });
        }
        Ok(Self {
            bindings: Cow::Owned(bindings),
            compositions: Cow::Owned(compositions),
            pending: [None; 4],
            pending_len: 0,
            caps_lock: false,
            num_lock: false,
        })
    }
    pub fn reset(&mut self) {
        self.pending = [None; 4];
        self.pending_len = 0;
    }
    pub fn is_composing(&self) -> bool {
        self.pending_len > 0
    }
    pub fn apply(&mut self, event: KeyEvent) -> Result<Option<KeymapText>, KeymapRefusal> {
        if event.transition() == KeyTransition::Released {
            return Ok(None);
        }
        if event.usage() == 0x29 {
            self.reset();
            return Ok(None);
        }
        let modifiers = event.modifiers_after().bits();
        if modifiers
            & (KeyModifiers::LEFT_CONTROL.bits()
                | KeyModifiers::RIGHT_CONTROL.bits()
                | KeyModifiers::LEFT_ALT.bits()
                | KeyModifiers::LEFT_GUI.bits()
                | KeyModifiers::RIGHT_GUI.bits())
            != 0
        {
            return Ok(None);
        }
        let shift =
            modifiers & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits()) != 0;
        let altgr = modifiers & KeyModifiers::RIGHT_ALT.bits() != 0;
        let lookup = |shift| {
            self.bindings
                .iter()
                .rev()
                .find(|binding| {
                    binding.usage == event.usage()
                        && binding.shift == shift
                        && binding.altgr == altgr
                })
                .map(|binding| binding.action)
                .unwrap_or(Action::None)
        };
        let mut action = lookup(shift);
        if self.caps_lock {
            if let (Action::Text(lower), Action::Text(upper)) = (lookup(false), lookup(true)) {
                let mut lower_chars = lower.as_str().chars();
                let mut upper_chars = upper.as_str().chars();
                if let (Some(lower), Some(upper)) = (lower_chars.next(), upper_chars.next()) {
                    let mut uppercase = lower.to_uppercase();
                    if lower_chars.next().is_none()
                        && upper_chars.next().is_none()
                        && lower != upper
                        && uppercase.next() == Some(upper)
                        && uppercase.next().is_none()
                    {
                        action = lookup(!shift);
                    }
                }
            }
        }
        if let Action::CapsText { ordinary, locked } = action {
            action = Action::Text(if self.caps_lock { locked } else { ordinary });
        }
        if let Action::Keypad(value) = action {
            action = if self.num_lock != shift {
                Action::Text(value)
            } else {
                Action::None
            };
        }
        match action {
            Action::None => Ok(None),
            Action::CapsLock => {
                self.caps_lock = !self.caps_lock;
                Ok(None)
            }
            Action::NumLock => {
                self.num_lock = !self.num_lock;
                Ok(None)
            }
            Action::CapsText { .. } => unreachable!("caps selection reduced before interpretation"),
            Action::Keypad(_) => unreachable!("keypad selection reduced before interpretation"),
            Action::Dead { spacing, combining } => {
                if self.pending[..self.pending_len]
                    .iter()
                    .flatten()
                    .any(|(_, mark)| *mark == combining)
                    || self.pending_len == 4
                {
                    let overflow = self.pending_len == 4;
                    let mut out = KeymapText::empty();
                    for (spacing, _) in self.pending[..self.pending_len].iter().flatten() {
                        out.push(spacing.as_str());
                    }
                    self.reset();
                    // Repeating a dead key emits its spacing form. Overflow flushes
                    // the finite prefix and starts a new prefix with this key.
                    if overflow {
                        self.pending[0] = Some((spacing, combining));
                        self.pending_len = 1;
                    }
                    Ok(Some(out))
                } else {
                    self.pending[self.pending_len] = Some((spacing, combining));
                    self.pending_len += 1;
                    Ok(None)
                }
            }
            Action::Text(value) => {
                let mut out = KeymapText::empty();
                if self.pending_len > 0 {
                    if value.as_str() == "\u{8}" {
                        self.reset();
                        return Ok(None);
                    }
                    if value.as_str() == " " {
                        for (spacing, _) in self.pending[..self.pending_len].iter().flatten() {
                            out.push(spacing.as_str());
                        }
                    } else if let Some(base) = value.as_str().chars().next() {
                        let mut scalars = ['\0'; 8];
                        let mut count = 0;
                        unicode_normalization::char::decompose_canonical(base, |scalar| {
                            scalars[count] = scalar;
                            count += 1;
                        });
                        for (_, mark) in self.pending[..self.pending_len].iter().flatten() {
                            scalars[count] = *mark;
                            count += 1;
                        }
                        // Canonical ordering preserves equal-class input order.
                        for index in 2..count {
                            let mut position = index;
                            while position > 1
                                && unicode_normalization::char::canonical_combining_class(
                                    scalars[position - 1],
                                ) > unicode_normalization::char::canonical_combining_class(
                                    scalars[position],
                                )
                            {
                                scalars.swap(position - 1, position);
                                position -= 1;
                            }
                        }
                        let mut starter = scalars[0];
                        let mut remaining = ['\0'; 7];
                        let mut rest = 0;
                        let mut previous_class = 0;
                        for mark in scalars[1..count].iter().copied() {
                            let class =
                                unicode_normalization::char::canonical_combining_class(mark);
                            let composed = self
                                .compositions
                                .iter()
                                .rev()
                                .find(|c| c.combining == mark && c.base == starter)
                                .map(|c| c.result)
                                .or_else(|| unicode_normalization::char::compose(starter, mark));
                            if let Some(result) =
                                composed.filter(|_| previous_class == 0 || previous_class < class)
                            {
                                starter = result;
                            } else {
                                remaining[rest] = mark;
                                rest += 1;
                                previous_class = class;
                            }
                        }
                        if rest == 0 {
                            out.character(starter);
                            out.push(&value.as_str()[base.len_utf8()..]);
                        } else {
                            // Preserve the familiar spacing fallback for an unsupported
                            // single dead key. A supported stacked prefix emits NFC text.
                            if self.pending_len == 1 && starter == base {
                                out.push(self.pending[0].expect("pending mark").0.as_str());
                                out.push(value.as_str());
                            } else {
                                out.character(starter);
                                for scalar in remaining[..rest].iter().copied() {
                                    out.character(scalar);
                                }
                                out.push(&value.as_str()[base.len_utf8()..]);
                            }
                        }
                    }
                    self.reset();
                } else {
                    out.push(value.as_str());
                }
                Ok((out.len > 0).then_some(out))
            }
        }
    }
}
fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, String> {
    match value.shape() {
        Shape::Record(fields) => fields
            .iter()
            .find(|f| f.name() == name)
            .map(|f| f.value())
            .ok_or_else(|| "missing keymap field".into()),
        _ => Err("expected keymap record".into()),
    }
}
fn collection(value: &StructuredInfoValue) -> Result<&[StructuredInfoValue], String> {
    match value.shape() {
        Shape::Collection(values) => Ok(values),
        _ => Err("expected finite keymap sequence".into()),
    }
}
fn leaf(value: &StructuredInfoValue) -> Result<&[u8], String> {
    match value.shape() {
        Shape::Leaf(bytes) => Ok(bytes),
        _ => Err("expected keymap leaf".into()),
    }
}
fn integer(value: &StructuredInfoValue) -> Result<u32, String> {
    let bytes = leaf(value)?;
    if bytes.is_empty() || bytes.len() > 4 {
        return Err("invalid keymap integer".into());
    }
    let mut word = [0; 4];
    word[..bytes.len()].copy_from_slice(bytes);
    Ok(u32::from_le_bytes(word))
}
fn scalar(value: &StructuredInfoValue) -> Result<char, String> {
    char::from_u32(integer(value)?).ok_or_else(|| "invalid keymap Unicode scalar".into())
}
fn boolean(value: &StructuredInfoValue) -> Result<bool, String> {
    match leaf(value)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err("invalid keymap Boolean".into()),
    }
}
fn text(value: &StructuredInfoValue) -> Result<&str, String> {
    core::str::from_utf8(leaf(value)?).map_err(|_| "invalid keymap UTF-8".to_string())
}

mod choices;
pub use choices::*;

#[cfg(test)]
mod representation_tests {
    use super::Text;

    #[test]
    fn admitted_utf8_text_uses_nine_bytes_without_pointer_sized_length_padding() {
        assert_eq!(core::mem::size_of::<Text>(), 9);
        for text in ["", "abcdefgh", "🦀🦀"] {
            assert_eq!(Text::new(text).unwrap().as_str(), text);
        }
        assert!(Text::new("abcdefghi").is_err());
        assert!(Text::new("🦀🦀x").is_err());
    }
}

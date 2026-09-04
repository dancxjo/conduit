//! Fixed-capacity renderer-neutral presentation obligations.

pub const PRESENTATION_COMPOSITION_KIND: &str = "presentation/composition@1";
pub const MAX_COMPOSITION_ITEMS: usize = 8;
pub const MAX_COMPOSITION_TOKEN_BYTES: usize = 32;
pub const MAX_COMPOSITION_NAME_BYTES: usize = 64;
pub const MAX_PRESENTATION_COMPOSITION_BYTES: usize =
    2 + MAX_COMPOSITION_ITEMS * (4 + MAX_COMPOSITION_TOKEN_BYTES + MAX_COMPOSITION_NAME_BYTES);
const VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationIconKey {
    Clock,
    Repeat2,
    Presentation,
    Type,
    CaseUpper,
    Combine,
    Tally5,
    ChartColumnsIncreasing,
    FileOutput,
    Keyboard,
    GenericGear,
}

impl PresentationIconKey {
    pub const ALL: [Self; 11] = [
        Self::Clock,
        Self::Repeat2,
        Self::Presentation,
        Self::Type,
        Self::CaseUpper,
        Self::Combine,
        Self::Tally5,
        Self::ChartColumnsIncreasing,
        Self::FileOutput,
        Self::Keyboard,
        Self::GenericGear,
    ];
    pub const ALL_UPSTREAM: [Self; 10] = [
        Self::Clock,
        Self::Repeat2,
        Self::Presentation,
        Self::Type,
        Self::CaseUpper,
        Self::Combine,
        Self::Tally5,
        Self::ChartColumnsIncreasing,
        Self::FileOutput,
        Self::Keyboard,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Repeat2 => "repeat-2",
            Self::Presentation => "presentation",
            Self::Type => "type",
            Self::CaseUpper => "case-upper",
            Self::Combine => "combine",
            Self::Tally5 => "tally-5",
            Self::ChartColumnsIncreasing => "chart-no-axes-column-increasing",
            Self::FileOutput => "file-output",
            Self::Keyboard => "keyboard",
            Self::GenericGear => "conduit-generic-gear",
        }
    }

    pub const fn accessibility_name(self) -> &'static str {
        match self {
            Self::Clock => "clock",
            Self::Repeat2 => "repeating flow",
            Self::Presentation => "presentation screen",
            Self::Type => "text",
            Self::CaseUpper => "uppercase letters",
            Self::Combine => "combined values",
            Self::Tally5 => "count tally",
            Self::ChartColumnsIncreasing => "count chart",
            Self::FileOutput => "file output",
            Self::Keyboard => "keyboard input",
            Self::GenericGear => "generic Gear; icon metadata missing",
        }
    }

    pub const fn is_fallback(self) -> bool {
        matches!(self, Self::GenericGear)
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|candidate| candidate.as_str() == token)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompositionItemKind {
    Icon = 1,
    Frame = 2,
    Badge = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessibilityRole {
    Image = 1,
    Group = 2,
    Status = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositionItem {
    pub kind: CompositionItemKind,
    pub role: AccessibilityRole,
    token_len: u8,
    name_len: u8,
    token: [u8; MAX_COMPOSITION_TOKEN_BYTES],
    name: [u8; MAX_COMPOSITION_NAME_BYTES],
}

impl CompositionItem {
    pub fn new(
        kind: CompositionItemKind,
        role: AccessibilityRole,
        token: &str,
        accessibility_name: &str,
    ) -> Result<Self, CompositionError> {
        if token.is_empty() || accessibility_name.is_empty() {
            return Err(CompositionError::EmptyText);
        }
        if token.len() > MAX_COMPOSITION_TOKEN_BYTES
            || accessibility_name.len() > MAX_COMPOSITION_NAME_BYTES
        {
            return Err(CompositionError::TextTooLong);
        }
        let mut item = Self {
            kind,
            role,
            token_len: token.len() as u8,
            name_len: accessibility_name.len() as u8,
            token: [0; MAX_COMPOSITION_TOKEN_BYTES],
            name: [0; MAX_COMPOSITION_NAME_BYTES],
        };
        item.token[..token.len()].copy_from_slice(token.as_bytes());
        item.name[..accessibility_name.len()].copy_from_slice(accessibility_name.as_bytes());
        Ok(item)
    }

    pub fn token(&self) -> &str {
        core::str::from_utf8(&self.token[..usize::from(self.token_len)])
            .expect("validated composition token")
    }

    pub fn accessibility_name(&self) -> &str {
        core::str::from_utf8(&self.name[..usize::from(self.name_len)])
            .expect("validated accessibility name")
    }
}

const EMPTY_ITEM: CompositionItem = CompositionItem {
    kind: CompositionItemKind::Icon,
    role: AccessibilityRole::Image,
    token_len: 0,
    name_len: 0,
    token: [0; MAX_COMPOSITION_TOKEN_BYTES],
    name: [0; MAX_COMPOSITION_NAME_BYTES],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationComposition {
    count: u8,
    items: [CompositionItem; MAX_COMPOSITION_ITEMS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionError {
    EmptyText,
    TextTooLong,
    TooManyItems,
    UnknownIcon,
    MalformedEncoding,
    NonCanonicalEncoding,
}

impl PresentationComposition {
    pub fn icon(token: &str, accessibility_name: &str) -> Result<Self, CompositionError> {
        if !is_authoritative_icon(token) {
            return Err(CompositionError::UnknownIcon);
        }
        let mut value = Self::empty();
        value.push(CompositionItem::new(
            CompositionItemKind::Icon,
            AccessibilityRole::Image,
            token,
            accessibility_name,
        )?)?;
        Ok(value)
    }

    pub fn icon_or_fallback(
        token: Option<&str>,
        accessibility_name: Option<&str>,
    ) -> Result<Self, CompositionError> {
        match (token, accessibility_name) {
            (Some(token), Some(name)) => Self::icon(token, name),
            (None, None) => Self::icon(
                "conduit-generic-gear",
                "generic Gear; icon metadata missing",
            ),
            _ => Err(CompositionError::MalformedEncoding),
        }
    }

    pub const fn empty() -> Self {
        Self {
            count: 0,
            items: [EMPTY_ITEM; MAX_COMPOSITION_ITEMS],
        }
    }

    pub fn push(&mut self, item: CompositionItem) -> Result<(), CompositionError> {
        let index = usize::from(self.count);
        if index == MAX_COMPOSITION_ITEMS {
            return Err(CompositionError::TooManyItems);
        }
        self.items[index] = item;
        self.count += 1;
        Ok(())
    }

    pub fn frame(mut self, role: &str, accessibility_name: &str) -> Result<Self, CompositionError> {
        self.push(CompositionItem::new(
            CompositionItemKind::Frame,
            AccessibilityRole::Group,
            role,
            accessibility_name,
        )?)?;
        Ok(self)
    }

    pub fn badge(
        mut self,
        state: &str,
        accessibility_name: &str,
    ) -> Result<Self, CompositionError> {
        self.push(CompositionItem::new(
            CompositionItemKind::Badge,
            AccessibilityRole::Status,
            state,
            accessibility_name,
        )?)?;
        Ok(self)
    }

    pub fn items(&self) -> &[CompositionItem] {
        &self.items[..usize::from(self.count)]
    }

    pub fn encoded_len(&self) -> usize {
        2 + self
            .items()
            .iter()
            .map(|item| 4 + usize::from(item.token_len) + usize::from(item.name_len))
            .sum::<usize>()
    }

    pub fn encode(&self) -> [u8; MAX_PRESENTATION_COMPOSITION_BYTES] {
        let mut output = [0; MAX_PRESENTATION_COMPOSITION_BYTES];
        output[0] = VERSION;
        output[1] = self.count;
        let mut cursor = 2;
        for item in self.items() {
            output[cursor..cursor + 4].copy_from_slice(&[
                item.kind as u8,
                item.role as u8,
                item.token_len,
                item.name_len,
            ]);
            cursor += 4;
            let token = item.token();
            output[cursor..cursor + token.len()].copy_from_slice(token.as_bytes());
            cursor += token.len();
            let name = item.accessibility_name();
            output[cursor..cursor + name.len()].copy_from_slice(name.as_bytes());
            cursor += name.len();
        }
        output
    }

    pub fn decode(input: &[u8]) -> Result<Self, CompositionError> {
        if input.len() < 2 || input[0] != VERSION {
            return Err(CompositionError::MalformedEncoding);
        }
        let count = usize::from(input[1]);
        if count > MAX_COMPOSITION_ITEMS {
            return Err(CompositionError::TooManyItems);
        }
        let mut output = Self::empty();
        let mut cursor = 2;
        for _ in 0..count {
            let header = input
                .get(cursor..cursor + 4)
                .ok_or(CompositionError::MalformedEncoding)?;
            cursor += 4;
            let kind = decode_kind(header[0])?;
            let role = decode_role(header[1])?;
            let token_len = usize::from(header[2]);
            let name_len = usize::from(header[3]);
            let token = input
                .get(cursor..cursor + token_len)
                .ok_or(CompositionError::MalformedEncoding)?;
            cursor += token_len;
            let name = input
                .get(cursor..cursor + name_len)
                .ok_or(CompositionError::MalformedEncoding)?;
            cursor += name_len;
            let token =
                core::str::from_utf8(token).map_err(|_| CompositionError::MalformedEncoding)?;
            let name =
                core::str::from_utf8(name).map_err(|_| CompositionError::MalformedEncoding)?;
            let item = CompositionItem::new(kind, role, token, name)?;
            if !matches!(
                (kind, role),
                (CompositionItemKind::Icon, AccessibilityRole::Image)
                    | (CompositionItemKind::Frame, AccessibilityRole::Group)
                    | (CompositionItemKind::Badge, AccessibilityRole::Status)
            ) {
                return Err(CompositionError::NonCanonicalEncoding);
            }
            if kind == CompositionItemKind::Icon && !is_authoritative_icon(token) {
                return Err(CompositionError::UnknownIcon);
            }
            output.push(item)?;
        }
        if cursor != input.len() {
            return Err(CompositionError::NonCanonicalEncoding);
        }
        Ok(output)
    }
}

pub fn is_authoritative_icon(token: &str) -> bool {
    PresentationIconKey::from_token(token).is_some()
}

fn decode_kind(value: u8) -> Result<CompositionItemKind, CompositionError> {
    match value {
        1 => Ok(CompositionItemKind::Icon),
        2 => Ok(CompositionItemKind::Frame),
        3 => Ok(CompositionItemKind::Badge),
        _ => Err(CompositionError::MalformedEncoding),
    }
}

fn decode_role(value: u8) -> Result<AccessibilityRole, CompositionError> {
    match value {
        1 => Ok(AccessibilityRole::Image),
        2 => Ok(AccessibilityRole::Group),
        3 => Ok(AccessibilityRole::Status),
        _ => Err(CompositionError::MalformedEncoding),
    }
}

#[cfg(test)]
mod tests;

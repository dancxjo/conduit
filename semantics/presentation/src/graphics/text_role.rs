use super::GraphicsError;

/// Finite graphical typography purposes. Hosts choose their exact metrics and
/// pinned resources; these do not alter authored text or Presentation identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphicsTextRole {
    Body = 0,
    Label = 1,
    Heading = 2,
    Title = 3,
    Code = 4,
}

impl GraphicsTextRole {
    pub(super) fn decode(value: u8) -> Result<Self, GraphicsError> {
        match value {
            0 => Ok(Self::Body),
            1 => Ok(Self::Label),
            2 => Ok(Self::Heading),
            3 => Ok(Self::Title),
            4 => Ok(Self::Code),
            _ => Err(GraphicsError::MalformedEncoding),
        }
    }
}

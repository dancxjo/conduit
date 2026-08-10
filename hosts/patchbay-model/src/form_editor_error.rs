//! Exact canonical Form authoring failures.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormEditorError {
    NotCanonicalFormPath,
    SourceTooLarge,
    Catalog(String),
    StaleRevision { current: u64, offered: u64 },
    UnknownForm(String),
    GraphTooLarge,
    UnknownPaletteKind(String),
    InvalidGearName,
    UnknownGear(String),
    UnknownPort(String),
    IncompatiblePorts(String),
    DuplicateCord,
    NestedGearEditUnsupported(String),
    StaleGraphBasis,
}

impl std::fmt::Display for FormEditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCanonicalFormPath => f.write_str("canonical Form paths must end in .conduit"),
            Self::SourceTooLarge => {
                f.write_str("canonical Form source exceeds its finite byte bound")
            }
            Self::Catalog(message) => write!(f, "Form catalog error: {message}"),
            Self::StaleRevision { current, offered } => write!(
                f,
                "stale checked revision {offered} cannot replace current revision {current}"
            ),
            Self::UnknownForm(name) => write!(f, "checked Form has no reusable form '{name}'"),
            Self::GraphTooLarge => f.write_str("checked Form graph exceeds its finite item bound"),
            Self::UnknownPaletteKind(kind) => write!(f, "palette Kind '{kind}' is unavailable"),
            Self::InvalidGearName => f.write_str("generated Gear name is not canonical"),
            Self::UnknownGear(gear) => write!(f, "Gear '{gear}' is not in the open Form"),
            Self::UnknownPort(port) => write!(f, "Port '{port}' is not in the current typed Form"),
            Self::IncompatiblePorts(reason) => write!(f, "Ports cannot connect: {reason}"),
            Self::DuplicateCord => f.write_str("those Ports already have a Cord"),
            Self::NestedGearEditUnsupported(gear) => write!(
                f,
                "Gear '{gear}' is inside a reusable Face; edit that Face rather than its expansion"
            ),
            Self::StaleGraphBasis => {
                f.write_str("the visual edit names a stale expanded Form revision")
            }
        }
    }
}

impl std::error::Error for FormEditorError {}

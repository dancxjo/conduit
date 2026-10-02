//! Exact canonical Plot authoring failures.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlotEditorError {
    NotCanonicalPlotPath,
    SourceTooLarge,
    Catalog(String),
    StaleRevision { current: u64, offered: u64 },
    UnknownPlot(String),
    GraphTooLarge,
    UnknownPaletteKind(String),
    InvalidGearName,
    UnknownGear(String),
    UnknownPort(String),
    UnknownCord(String),
    IncompatiblePorts(String),
    DuplicateCord,
    NestedGearEditUnsupported(String),
    StaleGraphBasis,
    UnknownConfiguration(String),
    InvalidConfiguration(String),
}

impl std::fmt::Display for PlotEditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCanonicalPlotPath => f.write_str("canonical Plot paths must end in .conduit"),
            Self::SourceTooLarge => {
                f.write_str("canonical Plot source exceeds its finite byte bound")
            }
            Self::Catalog(message) => write!(f, "Plot catalog error: {message}"),
            Self::StaleRevision { current, offered } => write!(
                f,
                "stale checked revision {offered} cannot replace current revision {current}"
            ),
            Self::UnknownPlot(name) => write!(f, "checked plot has no reusable plot '{name}'"),
            Self::GraphTooLarge => f.write_str("checked plot graph exceeds its finite item bound"),
            Self::UnknownPaletteKind(kind) => write!(f, "palette Kind '{kind}' is unavailable"),
            Self::InvalidGearName => f.write_str("generated Gear name is not canonical"),
            Self::UnknownGear(gear) => write!(f, "Gear '{gear}' is not in the open Plot"),
            Self::UnknownPort(port) => write!(f, "Port '{port}' is not in the current typed Plot"),
            Self::UnknownCord(cord) => write!(f, "Cord '{cord}' is not in the current typed Plot"),
            Self::IncompatiblePorts(reason) => write!(f, "Ports cannot connect: {reason}"),
            Self::DuplicateCord => f.write_str("those Ports already have a Cord"),
            Self::NestedGearEditUnsupported(gear) => write!(
                f,
                "Gear '{gear}' is inside a reusable Front; edit that front rather than its expansion"
            ),
            Self::StaleGraphBasis => {
                f.write_str("the visual edit names a stale expanded Plot revision")
            }
            Self::UnknownConfiguration(key) => {
                write!(f, "Gear configuration '{key}' is unavailable")
            }
            Self::InvalidConfiguration(reason) => write!(f, "configuration edit refused: {reason}"),
        }
    }
}

impl std::error::Error for PlotEditorError {}

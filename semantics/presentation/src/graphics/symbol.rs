//! Finite graphical symbols shared by native Presenters.
//!
//! These are graphical resources, not portable Presentation identities. Each
//! renderer supplies bounded geometry; accessible names remain independent of
//! color, pixel size, and platform assets.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsSymbol {
    Form,
    Build,
    Body,
    Wake,
    Lull,
    Plan,
    Play,
    Stop,
    Hold,
    Host,
    Gear,
    PortInput,
    PortOutput,
    Cord,
    Line,
    Sign,
    Info,
    Face,
    Back,
    Warning,
    Failure,
    Success,
    Open,
    Save,
    Inspect,
}

impl GraphicsSymbol {
    pub const ALL: [Self; 25] = [
        Self::Form,
        Self::Build,
        Self::Body,
        Self::Wake,
        Self::Lull,
        Self::Plan,
        Self::Play,
        Self::Stop,
        Self::Hold,
        Self::Host,
        Self::Gear,
        Self::PortInput,
        Self::PortOutput,
        Self::Cord,
        Self::Line,
        Self::Sign,
        Self::Info,
        Self::Face,
        Self::Back,
        Self::Warning,
        Self::Failure,
        Self::Success,
        Self::Open,
        Self::Save,
        Self::Inspect,
    ];

    pub const fn accessibility_name(self) -> &'static str {
        match self {
            Self::Form => "Form",
            Self::Build => "Build",
            Self::Body => "Body",
            Self::Wake => "Wake",
            Self::Lull => "Lull",
            Self::Plan => "Plan",
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::Hold => "Hold",
            Self::Host => "Host",
            Self::Gear => "Gear",
            Self::PortInput => "Input Port",
            Self::PortOutput => "Output Port",
            Self::Cord => "Cord",
            Self::Line => "Line",
            Self::Sign => "Sign",
            Self::Info => "Info",
            Self::Face => "Face",
            Self::Back => "Back",
            Self::Warning => "Warning",
            Self::Failure => "Failure",
            Self::Success => "Success",
            Self::Open => "Open",
            Self::Save => "Save",
            Self::Inspect => "Inspect",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GraphicsSymbol;

    #[test]
    fn symbols_are_finite_and_have_distinct_accessible_names() {
        assert_eq!(GraphicsSymbol::ALL.len(), 25);
        for (index, symbol) in GraphicsSymbol::ALL.iter().enumerate() {
            assert!(!symbol.accessibility_name().is_empty());
            for other in &GraphicsSymbol::ALL[..index] {
                assert_ne!(symbol, other);
                assert_ne!(symbol.accessibility_name(), other.accessibility_name());
            }
        }
    }
}

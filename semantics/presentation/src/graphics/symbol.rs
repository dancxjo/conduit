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
    /// Stable graphical token; deliberately separate from palette icon keys.
    pub const fn as_token(self) -> &'static str {
        match self {
            Self::Form => "symbol/form",
            Self::Build => "symbol/build",
            Self::Body => "symbol/body",
            Self::Wake => "symbol/wake",
            Self::Lull => "symbol/lull",
            Self::Plan => "symbol/plan",
            Self::Play => "symbol/play",
            Self::Stop => "symbol/stop",
            Self::Hold => "symbol/hold",
            Self::Host => "symbol/host",
            Self::Gear => "symbol/gear",
            Self::PortInput => "symbol/port-input",
            Self::PortOutput => "symbol/port-output",
            Self::Cord => "symbol/cord",
            Self::Line => "symbol/line",
            Self::Sign => "symbol/sign",
            Self::Info => "symbol/info",
            Self::Face => "symbol/face",
            Self::Back => "symbol/back",
            Self::Warning => "symbol/warning",
            Self::Failure => "symbol/failure",
            Self::Success => "symbol/success",
            Self::Open => "symbol/open",
            Self::Save => "symbol/save",
            Self::Inspect => "symbol/inspect",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|symbol| symbol.as_token() == token)
    }

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
            assert_eq!(GraphicsSymbol::from_token(symbol.as_token()), Some(*symbol));
            for other in &GraphicsSymbol::ALL[..index] {
                assert_ne!(symbol, other);
                assert_ne!(symbol.accessibility_name(), other.accessibility_name());
            }
        }
        assert_eq!(GraphicsSymbol::from_token("Body"), None);
        assert_eq!(GraphicsSymbol::from_token("symbol/unknown"), None);
    }

    #[test]
    fn symbol_commands_round_trip_without_changing_their_identity() {
        use crate::{GraphicsCommand, GraphicsPaintRole, GraphicsScene, LayoutRect};
        let rect = LayoutRect {
            x: 0,
            y: 0,
            width: 24,
            height: 24,
        };
        for symbol in GraphicsSymbol::ALL {
            let mut scene = GraphicsScene::empty();
            scene
                .push(
                    GraphicsCommand::symbol(rect, rect, GraphicsPaintRole::Foreground, symbol)
                        .unwrap(),
                )
                .unwrap();
            let bytes = scene.encode();
            assert_eq!(
                GraphicsScene::decode(&bytes[..scene.encoded_len()]).unwrap(),
                scene
            );
        }
    }
}

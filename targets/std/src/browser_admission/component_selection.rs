//! Finite transport choices for existing checked Source presets, not Look data.
use conduit_face::{ComponentMaskLook, TodoComponentRecipe};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserComponentLook {
    Compact,
    Spacious,
    CompactEditorial,
    SpaciousPhosphor,
}

impl From<BrowserComponentLook> for ComponentMaskLook {
    fn from(value: BrowserComponentLook) -> Self {
        match value {
            BrowserComponentLook::Compact => Self::Compact,
            BrowserComponentLook::Spacious => Self::Spacious,
            BrowserComponentLook::CompactEditorial => Self::CompactEditorial,
            BrowserComponentLook::SpaciousPhosphor => Self::SpaciousPhosphor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserComponentRecipe {
    Flat,
    Grouped,
    RequiredItem,
}

impl From<BrowserComponentRecipe> for TodoComponentRecipe {
    fn from(value: BrowserComponentRecipe) -> Self {
        match value {
            BrowserComponentRecipe::Flat => Self::Flat,
            BrowserComponentRecipe::Grouped => Self::Grouped,
            BrowserComponentRecipe::RequiredItem => Self::RequiredItem,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{ComponentVisualSelection, VisualDensity, VisualPalette};
    #[test]
    fn browser_wire_covers_four_existing_independent_density_palette_roots() {
        for (wire, density, palette) in [
            ("compact", VisualDensity::Compact, VisualPalette::Phosphor),
            (
                "compact-editorial",
                VisualDensity::Compact,
                VisualPalette::Editorial,
            ),
            (
                "spacious-phosphor",
                VisualDensity::Spacious,
                VisualPalette::Phosphor,
            ),
            (
                "spacious",
                VisualDensity::Spacious,
                VisualPalette::Editorial,
            ),
        ] {
            let parsed: BrowserComponentLook =
                serde_json::from_value(serde_json::json!(wire)).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), wire);
            let selection = ComponentVisualSelection::from(ComponentMaskLook::from(parsed));
            assert_eq!(selection.density, density);
            assert_eq!(selection.palette, palette);
        }
        for invalid in [
            serde_json::json!("custom-css"),
            serde_json::json!("compact@2"),
            serde_json::json!({"density":"compact"}),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<BrowserComponentLook>(invalid).is_err());
        }
    }
}

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
    PatchbayNodes,
}

impl BrowserComponentRecipe {
    /// The selected checked component owns structure; the Look only changes paint.
    pub fn source_root(self, look: ComponentMaskLook) -> (&'static str, &'static str) {
        use conduit_face::{
            patchbay_component_mask_root, todo_component_mask_root, PATCHBAY_COMPONENT_MASK_SOURCE,
            TODO_COMPONENT_MASK_SOURCE,
        };
        match self {
            Self::PatchbayNodes => (
                PATCHBAY_COMPONENT_MASK_SOURCE,
                patchbay_component_mask_root(look),
            ),
            Self::Flat => (
                TODO_COMPONENT_MASK_SOURCE,
                todo_component_mask_root(look, TodoComponentRecipe::Flat),
            ),
            Self::Grouped => (
                TODO_COMPONENT_MASK_SOURCE,
                todo_component_mask_root(look, TodoComponentRecipe::Grouped),
            ),
            Self::RequiredItem => (
                TODO_COMPONENT_MASK_SOURCE,
                todo_component_mask_root(look, TodoComponentRecipe::RequiredItem),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::{ComponentVisualSelection, VisualDensity, VisualPalette};
    #[test]
    fn patchbay_selection_keeps_its_checked_structure_across_looks() {
        let recipe: BrowserComponentRecipe = serde_json::from_str("\"patchbay-nodes\"").unwrap();
        for look in [
            ComponentMaskLook::Compact,
            ComponentMaskLook::Spacious,
            ComponentMaskLook::CompactEditorial,
            ComponentMaskLook::SpaciousPhosphor,
        ] {
            let (source, root) = recipe.source_root(look);
            assert_eq!(source, conduit_face::PATCHBAY_COMPONENT_MASK_SOURCE);
            assert_eq!(root, conduit_face::patchbay_component_mask_root(look));
            assert_ne!(source, BrowserComponentRecipe::Grouped.source_root(look).0);
        }
        assert_eq!(serde_json::to_value(recipe).unwrap(), "patchbay-nodes");
    }

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

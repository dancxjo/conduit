//! Finite checked Source-root selection; this is not a Look wire format.
//! The selected root still supplies canonical VisualExpressionInfo for admission.
use super::{ComponentMaskLook, CrecheComponentRecipe, TodoComponentRecipe};
use crate::{VisualDensity, VisualPalette};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentVisualSelection {
    pub density: VisualDensity,
    pub palette: VisualPalette,
}
impl Default for ComponentVisualSelection {
    fn default() -> Self {
        Self {
            density: VisualDensity::Compact,
            palette: VisualPalette::Phosphor,
        }
    }
}
impl From<ComponentMaskLook> for ComponentVisualSelection {
    fn from(look: ComponentMaskLook) -> Self {
        match look {
            ComponentMaskLook::Compact => Self::default(),
            ComponentMaskLook::CompactEditorial => Self {
                density: VisualDensity::Compact,
                palette: VisualPalette::Editorial,
            },
            ComponentMaskLook::SpaciousPhosphor => Self {
                density: VisualDensity::Spacious,
                palette: VisualPalette::Phosphor,
            },
            ComponentMaskLook::Spacious => Self {
                density: VisualDensity::Spacious,
                palette: VisualPalette::Editorial,
            },
        }
    }
}
impl From<ComponentVisualSelection> for ComponentMaskLook {
    fn from(selection: ComponentVisualSelection) -> Self {
        use {VisualDensity::*, VisualPalette::*};
        match (selection.density, selection.palette) {
            (Compact, Phosphor) => Self::Compact,
            (Compact, Editorial) => Self::CompactEditorial,
            (Spacious, Phosphor) => Self::SpaciousPhosphor,
            (Spacious, Editorial) => Self::Spacious,
        }
    }
}
impl ComponentVisualSelection {
    /// Reverse only the exact reviewed Source roots; never infer a foreign root
    /// from a prefix, suffix, display label or renderer implementation name.
    pub fn from_todo_root(root: &str) -> Option<(Self, TodoComponentRecipe)> {
        for density in [VisualDensity::Compact, VisualDensity::Spacious] {
            for palette in [VisualPalette::Phosphor, VisualPalette::Editorial] {
                let selection = Self { density, palette };
                for recipe in [
                    TodoComponentRecipe::Flat,
                    TodoComponentRecipe::Grouped,
                    TodoComponentRecipe::RequiredItem,
                ] {
                    if selection.todo_root(recipe) == root {
                        return Some((selection, recipe));
                    }
                }
            }
        }
        None
    }

    pub const fn todo_root(self, recipe: TodoComponentRecipe) -> &'static str {
        use {TodoComponentRecipe::*, VisualDensity::*, VisualPalette::*};
        match (recipe, self.density, self.palette) {
            (RequiredItem, Compact, Phosphor) => "todo-mask-required-item-compact",
            (RequiredItem, Compact, Editorial) => "todo-mask-required-item-compact-editorial",
            (RequiredItem, Spacious, Phosphor) => "todo-mask-required-item-spacious-phosphor",
            (RequiredItem, Spacious, Editorial) => "todo-mask-required-item-spacious",
            (Flat, Compact, Phosphor) => "todo-mask-flat-compact",
            (Flat, Compact, Editorial) => "todo-mask-flat-compact-editorial",
            (Flat, Spacious, Phosphor) => "todo-mask-flat-spacious-phosphor",
            (Flat, Spacious, Editorial) => "todo-mask-flat-spacious",
            (Grouped, Compact, Phosphor) => "todo-mask-grouped-compact",
            (Grouped, Compact, Editorial) => "todo-mask-grouped-compact-editorial",
            (Grouped, Spacious, Phosphor) => "todo-mask-grouped-spacious-phosphor",
            (Grouped, Spacious, Editorial) => "todo-mask-grouped-spacious",
        }
    }
    pub const fn creche_root(self, recipe: CrecheComponentRecipe) -> &'static str {
        use {CrecheComponentRecipe::*, VisualDensity::*, VisualPalette::*};
        match (recipe, self.density, self.palette) {
            (SingleColumn, Compact, Phosphor) => "creche-mask-single",
            (SingleColumn, Compact, Editorial) => "creche-mask-single-compact-editorial",
            (SingleColumn, Spacious, Phosphor) => "creche-mask-single-spacious-phosphor",
            (SingleColumn, Spacious, Editorial) => "creche-mask-single-spacious",
            (ResponsiveColumns, Compact, Phosphor) => "creche-mask-columns",
            (ResponsiveColumns, Compact, Editorial) => "creche-mask-columns-compact-editorial",
            (ResponsiveColumns, Spacious, Phosphor) => "creche-mask-columns-spacious-phosphor",
            (ResponsiveColumns, Spacious, Editorial) => "creche-mask-columns-spacious",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_native_preset_names_keep_existing_values_and_cover_independent_axes() {
        for (wire, look) in [
            ("compact", ComponentMaskLook::Compact),
            ("spacious", ComponentMaskLook::Spacious),
            ("compact-editorial", ComponentMaskLook::CompactEditorial),
            ("spacious-phosphor", ComponentMaskLook::SpaciousPhosphor),
        ] {
            let encoded = serde_json::to_value(look).unwrap();
            assert_eq!(encoded, wire);
            assert_eq!(
                serde_json::from_value::<ComponentMaskLook>(encoded).unwrap(),
                look
            );
            assert_eq!(
                ComponentMaskLook::from(ComponentVisualSelection::from(look)),
                look
            );
        }
        for wire in [
            serde_json::json!("editorial"),
            serde_json::json!("compact_phosphor"),
            serde_json::json!({"palette":"editorial","density":"compact"}),
        ] {
            assert!(serde_json::from_value::<ComponentMaskLook>(wire).is_err());
        }
    }
}

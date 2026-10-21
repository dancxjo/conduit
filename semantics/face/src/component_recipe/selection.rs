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
            ComponentMaskLook::Spacious => Self {
                density: VisualDensity::Spacious,
                palette: VisualPalette::Editorial,
            },
        }
    }
}
impl ComponentVisualSelection {
    pub const fn todo_root(self, recipe: TodoComponentRecipe) -> &'static str {
        use {TodoComponentRecipe::*, VisualDensity::*, VisualPalette::*};
        match (recipe, self.density, self.palette) {
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

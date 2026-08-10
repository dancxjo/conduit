//! Categorized native presentation of the authoritative Gear palette.

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PaletteCategory, PatchbayTheme};

use crate::{
    gui::{GuiAction, HitTarget},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, rgb, text, PixelRect},
    palette_icon::draw_palette_icon,
};

pub(super) fn draw_palette<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    query: &str,
    mut y: i32,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let Ok(palette) = patchbay_model::GearPalette::standard() else {
        return;
    };
    let entries = palette.search(query).unwrap_or_default();
    let mut category: Option<PaletteCategory> = None;
    for entry in entries {
        if category != Some(entry.category) {
            category = Some(entry.category);
            text(
                target,
                Point::new(14, y),
                entry.category.label(),
                theme.text_secondary,
            );
            y += 17;
        }
        let bounds = PixelRect {
            x: 12,
            y,
            width: 150,
            height: 22,
        };
        frame_rect(target, bounds, theme.structure_secondary, 1);
        let fallback = draw_palette_icon(
            target,
            entry.icon,
            Point::new(17, y + 3),
            rgb(theme.emphasis),
        );
        text(
            target,
            Point::new(38, y + 4),
            &format!(
                "{} {}>{} c{}{}",
                entry.plain_name,
                entry.inputs.len(),
                entry.outputs.len(),
                entry.configuration.len(),
                if fallback { " !" } else { "" }
            ),
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: GuiAction::PlacePaletteKind(entry.kind_id.as_str().into()),
            shape: HitShape::Rect(bounds),
        });
        y += 23;
    }
}

//! Left-side navigation, actions, and finite authoritative Gear chooser.

use crate::{
    gui::{GuiAction, HitTarget},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, icon_label, text, PixelRect},
    icon::Icon,
    palette_state::PaletteChooser,
    palette_view::draw_palette,
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::PatchbayTheme;

pub(super) fn draw_navigator<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    palette: &PaletteChooser,
    visible_subject_count: usize,
    body_born: bool,
    parts_open: bool,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    text(target, Point::new(14, 66), "NAVIGATOR", theme.emphasis);
    for (index, (icon, label)) in [
        (Icon::Form, "Forms"),
        (Icon::Body, "Bodies"),
        (Icon::Host, "Parts (F12)"),
        (Icon::Sign, "Signs"),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 92 + index as i32 * 30;
        icon_label(target, icon, Point::new(14, y), label, theme.text_primary);
        if index == 2 && body_born {
            let bounds = PixelRect {
                x: 10,
                y: y - 4,
                width: 154,
                height: 25,
            };
            if parts_open {
                frame_rect(target, bounds, theme.focus, 2);
            }
            targets.push(HitTarget {
                action: GuiAction::TogglePartsView,
                shape: HitShape::Rect(bounds),
            });
        }
    }
    text(target, Point::new(14, 226), "ACTIONS", theme.emphasis);
    action_button(
        target,
        Icon::Open,
        "Open Back",
        246,
        GuiAction::OpenBack,
        theme,
        targets,
    );
    action_button(
        target,
        Icon::Save,
        "Save",
        278,
        GuiAction::SaveForm,
        theme,
        targets,
    );
    action_button(
        target,
        Icon::Inspect,
        "Details (F2)",
        310,
        GuiAction::ToggleLinearView,
        theme,
        targets,
    );
    let heading = if palette.search_active() {
        format!("PALETTE SEARCH /{}", palette.query())
    } else {
        "PALETTE  / TO SEARCH".into()
    };
    text(target, Point::new(14, 354), &heading, theme.emphasis);
    draw_palette(
        target,
        palette,
        PaletteChooser::keyboard_target(visible_subject_count),
        374,
        theme,
        targets,
    );
}

fn action_button<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    icon: Icon,
    label: &str,
    y: i32,
    action: GuiAction,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let bounds = PixelRect {
        x: 12,
        y,
        width: 150,
        height: 26,
    };
    frame_rect(target, bounds, theme.structure_secondary, 1);
    icon_label(
        target,
        icon,
        Point::new(18, y + 5),
        label,
        theme.text_primary,
    );
    targets.push(HitTarget {
        action,
        shape: HitShape::Rect(bounds),
    });
}

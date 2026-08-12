//! Finite renderer-local lifecycle action strip and hit targets.

use crate::{
    gui::{GuiAction, HitTarget, LifecycleContext},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, text, PixelRect},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::PatchbayTheme;

pub(super) fn draw_lifecycle_actions<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    lifecycle: &LifecycleContext,
    width: i32,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let start_x = (width - 386).max(720);
    for (index, action) in lifecycle
        .actions
        .iter()
        .filter(|action| action.enabled)
        .take(3)
        .enumerate()
    {
        let bounds = PixelRect {
            x: start_x + index as i32 * 126,
            y: 25,
            width: 118,
            height: 22,
        };
        frame_rect(target, bounds, theme.focus, 1);
        text(
            target,
            Point::new(bounds.x + 5, bounds.y + 6),
            &format!("{} [{}]", action.label, action.accelerator),
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: GuiAction::Lifecycle(action.action),
            shape: HitShape::Rect(bounds),
        });
    }
    if let Some(next) = lifecycle.actions.iter().find(|action| !action.enabled) {
        text(
            target,
            Point::new(start_x, 14),
            &format!("{}: {}", next.label, next.explanation),
            theme.text_secondary,
        );
    }
}

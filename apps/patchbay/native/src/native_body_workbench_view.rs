//! Renderer-local native Body-workbench document and pointer navigation.

use crate::{
    canvas::SoftwareCanvas,
    gui::{GuiAction, HitTarget},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, text, PixelRect},
    native_body_workbench::NativeWorkbenchDestination,
    render::draw_document,
};
use embedded_graphics::prelude::Point;
use patchbay_model::PHOSPHOR_THEME;

pub fn draw_body_workbench(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    lines: &[String],
    selected: NativeWorkbenchDestination,
    history_selection: usize,
    history_entries: usize,
) -> Vec<HitTarget> {
    draw_document(pixels, width, height, lines);
    let mut canvas = SoftwareCanvas::new(pixels, width, height);
    let theme = &PHOSPHOR_THEME;
    let y = i32::try_from(height).unwrap_or(i32::MAX).saturating_sub(40);
    let mut targets = Vec::with_capacity(3 + history_entries);
    for (index, (label, destination)) in [
        ("PROGRAM", NativeWorkbenchDestination::Program),
        ("BODY", NativeWorkbenchDestination::Body),
        ("HISTORY", NativeWorkbenchDestination::History),
    ]
    .into_iter()
    .enumerate()
    {
        let bounds = PixelRect {
            x: 18 + index as i32 * 142,
            y,
            width: 128,
            height: 26,
        };
        frame_rect(
            &mut canvas,
            bounds,
            if selected == destination {
                theme.focus
            } else {
                theme.structure_secondary
            },
            if selected == destination { 2 } else { 1 },
        );
        text(
            &mut canvas,
            Point::new(bounds.x + 12, bounds.y + 7),
            label,
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: GuiAction::SelectBodyWorkbench(destination),
            shape: HitShape::Rect(bounds),
        });
    }
    if selected == NativeWorkbenchDestination::History {
        for index in 0..history_entries {
            let bounds = PixelRect {
                x: i32::try_from(width).unwrap_or(i32::MAX).saturating_sub(126),
                y: 72 + index as i32 * 30,
                width: 108,
                height: 22,
            };
            frame_rect(
                &mut canvas,
                bounds,
                if index == history_selection {
                    theme.focus
                } else {
                    theme.structure_secondary
                },
                if index == history_selection { 2 } else { 1 },
            );
            text(
                &mut canvas,
                Point::new(bounds.x + 8, bounds.y + 5),
                &format!("SIGN {}", index + 1),
                theme.text_primary,
            );
            targets.push(HitTarget {
                action: GuiAction::SelectBodyHistory(index),
                shape: HitShape::Rect(bounds),
            });
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_document_tabs_are_visible_bounded_and_semantically_exact() {
        let mut pixels = vec![crate::BACKGROUND; 640 * 360];
        let targets = draw_body_workbench(
            &mut pixels,
            640,
            360,
            &["Roseau · Body / Signs".into()],
            NativeWorkbenchDestination::History,
            1,
            3,
        );
        assert_eq!(targets.len(), 6);
        assert!(targets.iter().any(|target| {
            target.action == GuiAction::SelectBodyWorkbench(NativeWorkbenchDestination::History)
                && target.contains(310.0, 330.0)
        }));
        assert!(targets.iter().any(|target| {
            target.action == GuiAction::SelectBodyHistory(1) && target.contains(530.0, 107.0)
        }));
        assert!(pixels.iter().any(|pixel| *pixel != crate::BACKGROUND));
    }
}

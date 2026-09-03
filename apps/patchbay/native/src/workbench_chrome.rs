//! Native Program/Body/History chrome and the extracted Patchbay header seam.

use crate::{
    gui::{HitTarget, LifecycleContext},
    gui_hit::{GuiAction, HitShape},
    gui_primitives::{frame_rect, icon_label, text, PixelRect},
    icon::Icon,
    lifecycle_flow::draw_lifecycle_flow,
    native_workbench::NativeBodyWorkbench,
};
use conduit_presentation::{PresentationAspect, PresentationPlace};
use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
};
use patchbay_model::{PatchbayGraph, PatchbayTheme};

use crate::gui::HEADER_HEIGHT;

pub(super) fn draw_program_header<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    breadcrumb: &str,
    lifecycle: &LifecycleContext,
    width: i32,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let clip = Rectangle::new(Point::zero(), Size::new(232, HEADER_HEIGHT as u32));
    let mut meaning = target.clipped(&clip);
    icon_label(
        &mut meaning,
        Icon::Form,
        Point::new(14, 10),
        &format!(
            "FORM  {}",
            if breadcrumb.is_empty() {
                graph.form_name.as_str()
            } else {
                breadcrumb
            }
        ),
        theme.emphasis,
    );
    draw_lifecycle_flow(target, lifecycle, width, theme, targets);
}

pub(super) fn draw_workbench_tabs<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    width: i32,
    workbench: &NativeBodyWorkbench,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let selected = (workbench.place(), workbench.aspect());
    let tabs = [
        (
            "PROGRAM",
            PresentationPlace::Program,
            PresentationAspect::Structure,
        ),
        (
            "BODY",
            PresentationPlace::Body,
            PresentationAspect::Structure,
        ),
        (
            "HISTORY",
            PresentationPlace::Body,
            PresentationAspect::Signs,
        ),
    ];
    let label_width = 112_u32;
    for (index, (label, place, aspect)) in tabs.into_iter().enumerate() {
        let bounds = PixelRect {
            x: 12 + index as i32 * (label_width as i32 + 8),
            y: 8,
            width: label_width,
            height: 34,
        };
        let color = if selected == (place, aspect) {
            theme.focus
        } else {
            theme.structure_secondary
        };
        frame_rect(target, bounds, color, 2);
        text(
            target,
            Point::new(bounds.x + 10, bounds.y + 12),
            label,
            color,
        );
        targets.push(HitTarget {
            action: GuiAction::ShowWorkbench { place, aspect },
            shape: HitShape::Rect(bounds),
        });
    }
    if width >= 720 {
        text(
            target,
            Point::new(380, 13),
            &format!(
                "{} · {:?} / {:?} · {:?}",
                workbench.frame().friendly_name,
                workbench.place(),
                workbench.aspect(),
                workbench.depth()
            ),
            theme.text_secondary,
        );
    }
}

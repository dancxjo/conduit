//! Native rendering for the two presentation faces of one semantic Gear.

use crate::{
    gui::{GearLayout, GuiAction, HitTarget},
    gui_face_controls::draw_face_controls,
    gui_hit::HitShape,
    gui_primitives::{fill_rect, frame_rect, rgb, text, PixelRect},
    icon::{draw_icon, Icon},
};
use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Primitive, Size},
    primitives::{Circle, PrimitiveStyle, Rectangle},
    Drawable,
};
use patchbay_model::{
    GearRealizationInspection, PatchbayGraph, PatchbayLayout, PatchbayTheme, RealizationDisposition,
};

pub(super) struct GearViewContext<'a> {
    pub(super) presentation_layout: &'a PatchbayLayout,
    pub(super) realization_plan: Option<&'a conduit_core::Plan>,
    pub(super) realization_hosts: &'a [conduit_core::HostAdvertisement],
    pub(super) face_control_focus: usize,
}

pub(super) fn draw_gear<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: &GearLayout<'_>,
    selected: Option<&str>,
    view: &GearViewContext<'_>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let is_selected = selected == Some(layout.gear.identity.as_str());
    fill_rect(target, layout.bounds, theme.surface);
    frame_rect(
        target,
        layout.bounds,
        if is_selected {
            theme.focus
        } else {
            theme.structure_primary
        },
        if is_selected { 2 } else { 1 },
    );
    let clip = Rectangle::new(
        Point::new(layout.bounds.x, layout.bounds.y),
        Size::new(layout.bounds.width, layout.bounds.height),
    );
    let mut target = target.clipped(&clip);
    draw_icon(
        &mut target,
        Icon::Gear,
        Point::new(layout.bounds.x + 10, layout.bounds.y + 9),
        rgb(theme.emphasis),
    );
    text(
        &mut target,
        Point::new(layout.bounds.x + 34, layout.bounds.y + 10),
        &match &layout.group {
            Some(group) => format!("{} [{group}]", layout.gear.gear_id.as_str()),
            None => layout.gear.gear_id.as_str().to_owned(),
        },
        theme.text_primary,
    );
    let reversed = view.presentation_layout.is_reversed(&layout.gear.identity);
    if reversed {
        draw_realization(
            &mut target,
            graph,
            layout,
            view.realization_plan,
            view.realization_hosts,
            theme,
            targets,
        );
    } else {
        text(
            &mut target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 29),
            &format!("FORM  {}", layout.gear.kind_id.as_str()),
            theme.emphasis,
        );
    }
    targets.push(HitTarget {
        action: select_action(graph, &layout.gear.identity),
        shape: HitShape::Rect(layout.bounds),
    });
    draw_flip_control(&mut target, graph, layout, reversed, theme, targets);
    if !reversed {
        draw_ports(&mut target, graph, layout, selected, theme, targets);
        draw_face_controls(
            &mut target,
            graph,
            layout.gear,
            layout.bounds,
            is_selected.then_some(view.face_control_focus),
            theme,
            targets,
        );
    }
}

fn draw_realization<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: &GearLayout<'_>,
    plan: Option<&conduit_core::Plan>,
    hosts: &[conduit_core::HostAdvertisement],
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let subject = graph
        .subject_ref(&layout.gear.identity)
        .expect("drawn Gear belongs to the exact graph");
    let Ok(inspection) = GearRealizationInspection::inspect(graph, &subject, plan, hosts) else {
        text(
            target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 32),
            "REALIZATION / NO PLAN",
            theme.text_secondary,
        );
        return;
    };
    if let Some(selected) = &inspection.selected {
        text(
            target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 31),
            "REALIZATION",
            theme.emphasis,
        );
        text(
            target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 47),
            selected.implementation_id.as_str(),
            theme.text_primary,
        );
        text(
            target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 63),
            &format!(
                "{} / {}",
                selected.host_id.as_str(),
                selected.boot_id.as_str()
            ),
            theme.text_secondary,
        );
    }
    let alternatives = inspection
        .alternatives
        .iter()
        .filter(|candidate| candidate.disposition == RealizationDisposition::Compatible)
        .count();
    if alternatives > 0 {
        let bounds = PixelRect {
            x: layout.bounds.x + 12,
            y: layout.bounds.y + 80,
            width: 112,
            height: 20,
        };
        frame_rect(target, bounds, theme.structure_secondary, 1);
        text(
            target,
            Point::new(bounds.x + 5, bounds.y + 4),
            &format!("NEXT IMPL ({alternatives})"),
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: GuiAction::PrewakeNextImplementation(subject),
            shape: HitShape::Rect(bounds),
        });
    }
}

fn draw_flip_control<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: &GearLayout<'_>,
    reversed: bool,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let bounds = PixelRect {
        x: layout.bounds.x + i32::try_from(layout.bounds.width).unwrap_or(i32::MAX) - 48,
        y: layout.bounds.y + 7,
        width: 40,
        height: 20,
    };
    frame_rect(target, bounds, theme.structure_secondary, 1);
    text(
        target,
        Point::new(bounds.x + 5, bounds.y + 4),
        if reversed { "↻ FORM" } else { "↻ REAL" },
        theme.text_primary,
    );
    targets.push(HitTarget {
        action: GuiAction::FlipGear(
            graph
                .subject_ref(&layout.gear.identity)
                .expect("drawn Gear belongs to the exact graph"),
        ),
        shape: HitShape::Rect(bounds),
    });
}

fn draw_ports<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: &GearLayout<'_>,
    selected: Option<&str>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    for (identity, point) in layout.inputs.iter().chain(&layout.outputs) {
        let selected_port = selected == Some(identity.as_str());
        let _ = Circle::with_center(*point, 9)
            .into_styled(PrimitiveStyle::with_fill(rgb(if selected_port {
                theme.focus
            } else {
                theme.structure_primary
            })))
            .draw(target);
        let port = layout
            .gear
            .inputs
            .iter()
            .chain(&layout.gear.outputs)
            .find(|port| port.identity == *identity)
            .expect("layout Ports come from the Gear");
        let label_x = if port.descriptor.direction == conduit_core::PortDirection::Input {
            point.x + 12
        } else {
            point.x - 72
        };
        text(
            target,
            Point::new(label_x, point.y - 7),
            port.descriptor.port_id.as_str(),
            theme.text_secondary,
        );
        targets.push(HitTarget {
            action: select_action(graph, identity),
            shape: HitShape::Rect(PixelRect {
                x: point.x - 10,
                y: point.y - 10,
                width: 20,
                height: 20,
            }),
        });
    }
}

fn select_action(graph: &PatchbayGraph, identity: &str) -> GuiAction {
    GuiAction::SelectSubject(
        graph
            .subject_ref(identity)
            .expect("drawn subject belongs to the exact graph"),
    )
}

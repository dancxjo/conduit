//! Collapsed authored-composition layout, rendering, and exact Port mapping.

use crate::{
    gui::{GuiAction, HitTarget},
    gui_hit::HitShape,
    gui_primitives::{frame_rect, text, PixelRect},
};
use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
};
use patchbay_model::{ApplicationTheme, PatchbayGraph};

const HEADER_HEIGHT: i32 = 52;
const NAV_WIDTH: i32 = 176;
const INSPECTOR_WIDTH: i32 = 284;
const NODE_WIDTH: i32 = 190;

#[derive(Clone)]
pub(super) struct CompositionLayout<'a> {
    pub(super) composition: &'a patchbay_model::PatchbayComposition,
    pub(super) bounds: PixelRect,
    pub(super) inputs: Vec<(String, Point)>,
    pub(super) outputs: Vec<(String, Point)>,
}

pub(super) fn layout_compositions(graph: &PatchbayGraph, width: i32) -> Vec<CompositionLayout<'_>> {
    let canvas_left = NAV_WIDTH + 116;
    let canvas_right = (width - INSPECTOR_WIDTH - 28).max(canvas_left + NODE_WIDTH);
    let columns = ((canvas_right - canvas_left) / (NODE_WIDTH + 64)).max(1) as usize;
    graph
        .compositions
        .iter()
        .enumerate()
        .map(|(index, composition)| {
            let x = canvas_left + (index % columns) as i32 * (NODE_WIDTH + 64);
            let y = HEADER_HEIGHT + 28 + (index / columns) as i32 * 128;
            CompositionLayout {
                composition,
                bounds: PixelRect {
                    x,
                    y,
                    width: NODE_WIDTH as u32,
                    height: 96,
                },
                inputs: composition
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(index, port)| {
                        (
                            port.identity.clone(),
                            Point::new(x, y + 48 + index as i32 * 18),
                        )
                    })
                    .collect(),
                outputs: composition
                    .outputs
                    .iter()
                    .enumerate()
                    .map(|(index, port)| {
                        (
                            port.identity.clone(),
                            Point::new(x + NODE_WIDTH, y + 48 + index as i32 * 18),
                        )
                    })
                    .collect(),
            }
        })
        .collect()
}

pub(super) fn composition_port_point(
    layouts: &[CompositionLayout<'_>],
    identity: &str,
) -> Option<Point> {
    layouts.iter().find_map(|layout| {
        layout
            .inputs
            .iter()
            .chain(&layout.outputs)
            .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
            .or_else(|| {
                layout
                    .composition
                    .input_bindings
                    .iter()
                    .chain(&layout.composition.output_bindings)
                    .find(|binding| binding.internal_port == identity)
                    .and_then(|binding| {
                        layout.inputs.iter().chain(&layout.outputs).find_map(
                            |(candidate, point)| {
                                (candidate == &binding.face_port).then_some(*point)
                            },
                        )
                    })
            })
    })
}

pub(super) fn draw_compositions<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layouts: &[CompositionLayout<'_>],
    theme: &ApplicationTheme,
    targets: &mut Vec<HitTarget>,
) {
    for layout in layouts {
        frame_rect(target, layout.bounds, theme.emphasis, 2);
        let clip = Rectangle::new(
            Point::new(layout.bounds.x, layout.bounds.y),
            Size::new(layout.bounds.width, layout.bounds.height),
        );
        let mut target = target.clipped(&clip);
        text(
            &mut target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 12),
            &format!(
                "{} : {}",
                layout.composition.gear_name, layout.composition.back_name
            ),
            theme.emphasis,
        );
        text(
            &mut target,
            Point::new(layout.bounds.x + 12, layout.bounds.y + 30),
            "DOUBLE-CLICK / ENTER TO OPEN",
            theme.text_secondary,
        );
        for (port, (_, point)) in layout.composition.inputs.iter().zip(&layout.inputs) {
            text(
                &mut target,
                Point::new(point.x + 8, point.y - 3),
                port.descriptor.port_id.as_str(),
                theme.text_primary,
            );
            push_port_target(graph, targets, &port.identity, *point);
        }
        for (port, (_, point)) in layout.composition.outputs.iter().zip(&layout.outputs) {
            text(
                &mut target,
                Point::new(point.x - 70, point.y - 3),
                port.descriptor.port_id.as_str(),
                theme.text_primary,
            );
            push_port_target(graph, targets, &port.identity, *point);
        }
        targets.push(HitTarget {
            action: select_action(graph, &layout.composition.identity),
            shape: HitShape::Rect(layout.bounds),
        });
    }
}

fn push_port_target(
    graph: &PatchbayGraph,
    targets: &mut Vec<HitTarget>,
    identity: &str,
    point: Point,
) {
    targets.push(HitTarget {
        action: select_action(graph, identity),
        shape: HitShape::Rect(PixelRect {
            x: point.x - 6,
            y: point.y - 6,
            width: 12,
            height: 12,
        }),
    });
}

fn select_action(graph: &PatchbayGraph, identity: &str) -> GuiAction {
    GuiAction::SelectSubject(
        graph
            .subject_ref(identity)
            .expect("drawn composition subject belongs to the exact graph"),
    )
}

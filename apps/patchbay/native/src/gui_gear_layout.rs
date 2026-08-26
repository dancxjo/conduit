//! Finite renderer-local Gear geometry derived from typed graph and presentation layout.

use crate::{gui::GearLayout, gui_primitives::PixelRect};
use embedded_graphics::prelude::Point;
use patchbay_model::{PatchbayGear, PatchbayGraph};

pub(super) struct GearGeometry {
    pub(super) canvas_left: i32,
    pub(super) inspector_width: i32,
    pub(super) header_height: i32,
    pub(super) node_width: i32,
    pub(super) minimum_node_height: i32,
}

pub(super) fn layout_gears<'a>(
    graph: &'a PatchbayGraph,
    width: i32,
    presentation_layout: &patchbay_model::PatchbayLayout,
    geometry: GearGeometry,
) -> Vec<GearLayout<'a>> {
    let GearGeometry {
        canvas_left,
        inspector_width,
        header_height,
        node_width,
        minimum_node_height,
    } = geometry;
    let canvas_right = (width - inspector_width - 28).max(canvas_left + node_width);
    let columns = ((canvas_right - canvas_left) / (node_width + 64)).max(1) as usize;
    graph
        .gears
        .iter()
        .filter(|gear| graph.compositions.is_empty() || gear.source_form == graph.form_name)
        .enumerate()
        .map(|(index, gear)| {
            let column = index % columns;
            let row = index / columns;
            let default_x = canvas_left + column as i32 * (node_width + 64);
            let prior_height = graph
                .gears
                .chunks(columns)
                .take(row)
                .map(|gears| {
                    gears
                        .iter()
                        .map(|gear| gear_height(gear, minimum_node_height))
                        .max()
                        .unwrap_or(0)
                        + 36
                })
                .sum::<i32>();
            let default_y = header_height
                + 28
                + prior_height
                + if graph.compositions.is_empty() {
                    0
                } else {
                    132
                };
            let (x, y) = presentation_layout
                .position(&gear.identity)
                .unwrap_or((default_x, default_y));
            GearLayout {
                gear,
                bounds: PixelRect {
                    x,
                    y,
                    width: node_width as u32,
                    height: gear_height(gear, minimum_node_height) as u32,
                },
                inputs: port_points(&gear.inputs, x, y),
                outputs: port_points(&gear.outputs, x + node_width, y),
                group: presentation_layout
                    .gears
                    .iter()
                    .find(|placement| placement.gear_identity == gear.identity)
                    .and_then(|placement| placement.group.clone()),
            }
        })
        .collect()
}

fn gear_height(gear: &PatchbayGear, minimum: i32) -> i32 {
    let port_rows = gear.inputs.len().max(gear.outputs.len()) as i32;
    minimum.max(62 + port_rows * 18 + gear.controls.len() as i32 * 40)
}

fn port_points(ports: &[patchbay_model::PatchbayPort], x: i32, y: i32) -> Vec<(String, Point)> {
    ports
        .iter()
        .enumerate()
        .map(|(index, port)| {
            let offset = 48 + index as i32 * 18;
            (port.identity.clone(), Point::new(x, y + offset))
        })
        .collect()
}

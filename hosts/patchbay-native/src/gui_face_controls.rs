//! Compact control rendering and hit actions inside a semantic Gear Face.

use crate::{
    gui_hit::{GuiAction, HitShape, HitTarget},
    gui_primitives::{text, PixelRect},
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PatchbayGear, PatchbayGraph, PatchbayTheme};

pub(super) fn draw_face_controls<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    gear: &PatchbayGear,
    gear_bounds: PixelRect,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let port_rows = gear.inputs.len().max(gear.outputs.len()) as i32;
    let first_y = gear_bounds.y + 52 + port_rows * 18;
    let subject = graph
        .subject_ref(&gear.identity)
        .expect("drawn Gear belongs to the exact graph");
    for (index, control) in gear.controls.iter().enumerate() {
        let y = first_y + index as i32 * 22;
        text(
            target,
            Point::new(gear_bounds.x + 10, y),
            &format!(
                "{}={} [{}]",
                control.key,
                displayed_value(&control.value),
                displayed_contract(&control.kind)
            ),
            theme.text_secondary,
        );
        for (side, value) in control_actions(control).into_iter().enumerate() {
            let bounds = PixelRect {
                x: gear_bounds.x + 8 + side as i32 * 86,
                y: y - 3,
                width: 84,
                height: 18,
            };
            targets.push(HitTarget {
                action: GuiAction::ConfigureGear {
                    subject: subject.clone(),
                    key: control.key.clone(),
                    value,
                },
                shape: HitShape::Rect(bounds),
            });
        }
    }
}

fn displayed_value(value: &conduit_core::ConfigurationValue) -> String {
    match value {
        conduit_core::ConfigurationValue::Bool(value) => value.to_string(),
        conduit_core::ConfigurationValue::U64(value) => value.to_string(),
        conduit_core::ConfigurationValue::I64(value) => value.to_string(),
        conduit_core::ConfigurationValue::Text(value) => format!("\"{value}\""),
    }
}

fn displayed_contract(kind: &patchbay_model::FaceControlKind) -> String {
    match kind {
        patchbay_model::FaceControlKind::BooleanChoice { choices } => {
            format!("{}|{}", choices[0], choices[1])
        }
        patchbay_model::FaceControlKind::TextChoice { choices } => choices.join("|"),
        patchbay_model::FaceControlKind::Number {
            minimum,
            maximum,
            unit,
        }
        | patchbay_model::FaceControlKind::Range {
            minimum,
            maximum,
            unit,
        } => format!("{minimum}..{maximum}{}", unit.unwrap_or("")),
        patchbay_model::FaceControlKind::ScalarNumber {
            minimum,
            maximum,
            unit,
        } => format!("{minimum}..{maximum}{unit}"),
        patchbay_model::FaceControlKind::ShortText { maximum_bytes } => {
            format!("max {maximum_bytes}B")
        }
    }
}

fn control_actions(control: &patchbay_model::FaceControl) -> Vec<conduit_core::ConfigurationValue> {
    match (&control.kind, &control.value) {
        (
            patchbay_model::FaceControlKind::BooleanChoice { .. },
            conduit_core::ConfigurationValue::Bool(value),
        ) => vec![conduit_core::ConfigurationValue::Bool(!value)],
        (
            patchbay_model::FaceControlKind::TextChoice { choices },
            conduit_core::ConfigurationValue::Text(value),
        ) => choices
            .iter()
            .find(|choice| *choice != value)
            .cloned()
            .map(conduit_core::ConfigurationValue::Text)
            .into_iter()
            .collect(),
        (
            patchbay_model::FaceControlKind::Number {
                minimum, maximum, ..
            }
            | patchbay_model::FaceControlKind::Range {
                minimum, maximum, ..
            },
            conduit_core::ConfigurationValue::U64(value),
        ) => vec![
            conduit_core::ConfigurationValue::U64(value.saturating_sub(1).max(*minimum)),
            conduit_core::ConfigurationValue::U64(value.saturating_add(1).min(*maximum)),
        ],
        (
            patchbay_model::FaceControlKind::ScalarNumber {
                minimum, maximum, ..
            },
            conduit_core::ConfigurationValue::I64(value),
        ) => vec![
            conduit_core::ConfigurationValue::I64(value.saturating_sub(1).max(*minimum)),
            conduit_core::ConfigurationValue::I64(value.saturating_add(1).min(*maximum)),
        ],
        (
            patchbay_model::FaceControlKind::ShortText { .. },
            conduit_core::ConfigurationValue::Text(value),
        ) => vec![conduit_core::ConfigurationValue::Text(
            if value.is_empty() {
                "text".into()
            } else {
                String::new()
            },
        )],
        _ => Vec::new(),
    }
}

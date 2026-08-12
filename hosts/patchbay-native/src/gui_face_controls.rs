//! Compact control rendering and hit actions inside a semantic Gear Face.

use crate::{
    gui_hit::{GuiAction, HitShape, HitTarget},
    gui_primitives::{frame_rect, text, PixelRect},
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
    focused_action: Option<usize>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let port_rows = gear.inputs.len().max(gear.outputs.len()) as i32;
    let first_y = gear_bounds.y + 52 + port_rows * 18;
    let subject = graph
        .subject_ref(&gear.identity)
        .expect("drawn Gear belongs to the exact graph");
    let mut action_index = 0usize;
    for (index, control) in gear.controls.iter().enumerate() {
        let y = first_y + index as i32 * 40;
        text(
            target,
            Point::new(gear_bounds.x + 10, y),
            &format!(
                "{}={}  {}",
                control.key,
                displayed_value(&control.value),
                displayed_contract(&control.kind)
            ),
            theme.text_secondary,
        );
        let actions = control_actions(control);
        for (side, value) in actions.iter().cloned().enumerate() {
            let bounds = PixelRect {
                x: gear_bounds.x + 8 + side as i32 * 86,
                y: y + 15,
                width: 84,
                height: 20,
            };
            let focused = focused_action == Some(action_index);
            frame_rect(
                target,
                bounds,
                if focused {
                    theme.focus
                } else {
                    theme.structure_secondary
                },
                if focused { 2 } else { 1 },
            );
            text(
                target,
                Point::new(bounds.x + 5, bounds.y + 4),
                &action_label(control, &value, side, actions.len()),
                theme.text_primary,
            );
            targets.push(HitTarget {
                action: GuiAction::ConfigureGear {
                    subject: subject.clone(),
                    key: control.key.clone(),
                    value,
                },
                shape: HitShape::Rect(bounds),
            });
            action_index += 1;
        }
    }
}

pub(super) fn focused_face_action(
    graph: &PatchbayGraph,
    subject_identity: &str,
    focused: usize,
) -> Option<GuiAction> {
    let gear = graph
        .gears
        .iter()
        .find(|gear| gear.identity == subject_identity)?;
    let subject = graph.subject_ref(subject_identity).ok()?;
    gear.controls
        .iter()
        .flat_map(|control| {
            control_actions(control)
                .into_iter()
                .map(|value| GuiAction::ConfigureGear {
                    subject: subject.clone(),
                    key: control.key.clone(),
                    value,
                })
        })
        .nth(focused)
}

pub(super) fn face_action_count(graph: &PatchbayGraph, subject_identity: &str) -> usize {
    graph
        .gears
        .iter()
        .find(|gear| gear.identity == subject_identity)
        .map(|gear| {
            gear.controls
                .iter()
                .map(|control| control_actions(control).len())
                .sum()
        })
        .unwrap_or(0)
}

fn action_label(
    control: &patchbay_model::FaceControl,
    value: &conduit_core::ConfigurationValue,
    side: usize,
    count: usize,
) -> String {
    match control.kind {
        patchbay_model::FaceControlKind::Number { .. }
        | patchbay_model::FaceControlKind::Range { .. }
        | patchbay_model::FaceControlKind::ScalarNumber { .. } => {
            let marker = if side == 0 { "−" } else { "+" };
            format!("{marker} {}", displayed_value(value))
        }
        patchbay_model::FaceControlKind::BooleanChoice { .. } => {
            format!("☐ {}", displayed_value(value))
        }
        patchbay_model::FaceControlKind::TextChoice { .. } => {
            format!("▾ {}", displayed_value(value))
        }
        patchbay_model::FaceControlKind::ShortText { .. } => {
            let prefix = if count == 1 { "EDIT" } else { "SET" };
            format!("{prefix} {}", displayed_value(value))
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
        ) => (!value.is_empty())
            .then(|| conduit_core::ConfigurationValue::Text(String::new()))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

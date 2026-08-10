//! Canonical selected-subject inspection for the native GUI.

use crate::{
    gui_primitives::{icon_label, text},
    icon::Icon,
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PatchbayGraph, PatchbayTheme};

pub(super) fn draw_inspector<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    selected: Option<&str>,
    width: i32,
    inspector_width: i32,
    theme: &PatchbayTheme,
) {
    let x = width - inspector_width + 14;
    icon_label(
        target,
        Icon::Inspect,
        Point::new(x, 66),
        "INSPECTOR",
        theme.emphasis,
    );
    text(
        target,
        Point::new(x, 94),
        "CANONICAL SUBJECT",
        theme.text_secondary,
    );
    match selected.and_then(|identity| graph.inspect(identity).ok()) {
        Some(inspection) => {
            text(
                target,
                Point::new(x, 116),
                &format!("{:?}", inspection.subject_kind),
                theme.focus,
            );
            wrapped_text(
                target,
                Point::new(x, 138),
                &inspection.subject_identity,
                32,
                5,
                theme.text_primary,
            );
            for (index, fact) in inspection.exact_facts.iter().enumerate() {
                text(
                    target,
                    Point::new(x, 238 + index as i32 * 20),
                    fact,
                    theme.text_secondary,
                );
            }
        }
        None => text(
            target,
            Point::new(x, 116),
            "Select a Gear, Port, or Cord",
            theme.text_secondary,
        ),
    }
    text(target, Point::new(x, 330), "IDENTITY BASIS", theme.emphasis);
    identity_value(
        target,
        Point::new(x, 354),
        "source",
        graph.source_document_id.as_str(),
        theme,
    );
    identity_value(
        target,
        Point::new(x, 414),
        "checked",
        graph.checked_form_id.as_str(),
        theme,
    );
    identity_value(
        target,
        Point::new(x, 474),
        "expanded",
        graph.expanded_form_id.as_str(),
        theme,
    );
}

fn identity_value<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    origin: Point,
    label: &str,
    value: &str,
    theme: &PatchbayTheme,
) {
    text(target, origin, label, theme.emphasis);
    wrapped_text(
        target,
        origin + Point::new(0, 20),
        value,
        32,
        2,
        theme.text_secondary,
    );
}

fn wrapped_text<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    origin: Point,
    value: &str,
    columns: usize,
    rows: usize,
    color: patchbay_model::ThemeColor,
) {
    let characters = value.chars().collect::<Vec<_>>();
    for (row, chunk) in characters.chunks(columns).take(rows).enumerate() {
        let line = chunk.iter().collect::<String>();
        text(
            target,
            origin + Point::new(0, row as i32 * 18),
            &line,
            color,
        );
    }
}

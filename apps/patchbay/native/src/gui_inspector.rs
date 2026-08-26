//! Human-first selected-subject inspection with explicit exact-evidence disclosure.

use crate::{gui::LifecycleContext, interaction_status::InteractionStatus};

pub(super) struct InspectorView<'a> {
    pub(super) selected: Option<&'a str>,
    pub(super) palette: &'a crate::palette_state::PaletteChooser,
    pub(super) lifecycle: &'a LifecycleContext,
    pub(super) status: Option<&'a InteractionStatus>,
    pub(super) exact_open: bool,
    pub(super) width: i32,
    pub(super) inspector_width: i32,
}
use crate::{
    gui_hit::{GuiAction, HitShape, HitTarget},
    gui_primitives::{frame_rect, icon_label, text, PixelRect},
    icon::Icon,
};
use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point},
};
use patchbay_model::{PatchbayGraph, PatchbaySubjectKind, PatchbayTheme};

pub(super) fn draw_inspector<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    view: InspectorView<'_>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let InspectorView {
        selected,
        palette,
        lifecycle,
        status,
        exact_open,
        width,
        inspector_width,
    } = view;
    let x = width - inspector_width + 14;
    icon_label(
        target,
        Icon::Inspect,
        Point::new(x, 66),
        "INSPECTOR",
        theme.emphasis,
    );
    if palette.search_active() {
        draw_palette_inspection(target, graph, palette, x, theme);
        return;
    }
    let Some(inspection) = selected.and_then(|identity| graph.inspect(identity).ok()) else {
        // Empty selection is intentionally quiet: the canvas already teaches selection.
        return;
    };

    text(
        target,
        Point::new(x, 96),
        human_kind(inspection.subject_kind),
        theme.focus,
    );
    for (index, fact) in inspection.exact_facts.iter().take(5).enumerate() {
        text(
            target,
            Point::new(x, 120 + index as i32 * 20),
            fact,
            if index == 0 {
                theme.text_primary
            } else {
                theme.text_secondary
            },
        );
    }
    text(
        target,
        Point::new(x, 216),
        &format!("LIFECYCLE  {}", lifecycle.flow.state_text),
        theme.text_secondary,
    );
    if let Some(status) = status {
        text(
            target,
            Point::new(x, 236),
            &format!("STATUS  {}", status.text),
            theme.text_secondary,
        );
    }
    text(
        target,
        Point::new(x, if status.is_some() { 258 } else { 238 }),
        meaningful_action(inspection.subject_kind),
        theme.emphasis,
    );

    let disclosure = PixelRect {
        x,
        y: if status.is_some() { 282 } else { 262 },
        width: 252,
        height: 26,
    };
    frame_rect(
        target,
        disclosure,
        if exact_open {
            theme.focus
        } else {
            theme.structure_secondary
        },
        if exact_open { 2 } else { 1 },
    );
    text(
        target,
        Point::new(x + 7, disclosure.y + 7),
        if exact_open {
            "EXACT IDENTITY ▾  (Ctrl+I)"
        } else {
            "EXACT IDENTITY ▸  (Ctrl+I)"
        },
        theme.text_primary,
    );
    targets.push(HitTarget {
        action: GuiAction::ToggleExactIdentity,
        shape: HitShape::Rect(disclosure),
    });

    if exact_open {
        wrapped_text(
            target,
            Point::new(x, disclosure.y + 40),
            &inspection.subject_identity,
            32,
            3,
            theme.text_primary,
        );
        identity_value(
            target,
            Point::new(x, disclosure.y + 102),
            "source",
            graph.source_document_id.as_str(),
            theme,
        );
        identity_value(
            target,
            Point::new(x, disclosure.y + 160),
            "checked",
            graph.checked_form_id.as_str(),
            theme,
        );
        identity_value(
            target,
            Point::new(x, disclosure.y + 218),
            "expanded",
            graph.expanded_form_id.as_str(),
            theme,
        );
        wrapped_text(
            target,
            Point::new(x, disclosure.y + 276),
            &lifecycle.flow.exact_basis,
            32,
            3,
            theme.text_secondary,
        );
    }
}

fn draw_palette_inspection<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    chooser: &crate::palette_state::PaletteChooser,
    x: i32,
    theme: &PatchbayTheme,
) {
    let Ok(kind) = chooser.selected_kind() else {
        text(
            target,
            Point::new(x, 96),
            "NO MATCHING GEARS",
            theme.failure,
        );
        text(
            target,
            Point::new(x, 118),
            "Edit the query or press Escape.",
            theme.text_secondary,
        );
        return;
    };
    let Ok(palette) = patchbay_model::GearPalette::standard() else {
        text(
            target,
            Point::new(x, 96),
            "CATALOG UNAVAILABLE",
            theme.failure,
        );
        return;
    };
    let Some(entry) = palette.find(&conduit_core::KindId::from(kind)) else {
        return;
    };
    text(target, Point::new(x, 96), &entry.plain_name, theme.focus);
    text(
        target,
        Point::new(x, 118),
        entry.category.label(),
        theme.emphasis,
    );
    wrapped_text(
        target,
        Point::new(x, 140),
        &entry.summary,
        32,
        2,
        theme.text_primary,
    );
    identity_value(
        target,
        Point::new(x, 184),
        "exact Kind",
        entry.kind_id.as_str(),
        theme,
    );
    identity_value(
        target,
        Point::new(x, 242),
        "typed inputs",
        &port_contracts(&entry.inputs),
        theme,
    );
    identity_value(
        target,
        Point::new(x, 300),
        "typed outputs",
        &port_contracts(&entry.outputs),
        theme,
    );
    identity_value(
        target,
        Point::new(x, 358),
        "configuration",
        &configuration_contracts(&entry.configuration),
        theme,
    );
    identity_value(
        target,
        Point::new(x, 416),
        "finite limits",
        &format!(
            "active={} queue-items={} queue-bytes={}",
            entry.limits.max_active_instances,
            entry.limits.max_queue_items,
            entry.limits.max_queue_bytes
        ),
        theme,
    );
    let target_text = crate::palette_state::PaletteChooser::keyboard_target(
        graph.gears.len() + graph.compositions.len(),
    )
    .map(|(x, y)| format!("ENTER adds at visible target {x}, {y}"))
    .unwrap_or_else(|error| error.message().to_owned());
    text(target, Point::new(x, 478), &target_text, theme.emphasis);
}

fn port_contracts(ports: &[conduit_core::PortDescriptor]) -> String {
    if ports.is_empty() {
        return "none".into();
    }
    ports
        .iter()
        .map(|port| format!("{}:{}", port.port_id.as_str(), port.value_kind.as_str()))
        .collect::<Vec<_>>()
        .join("  ")
}

fn configuration_contracts(fields: &[patchbay_model::PaletteConfigurationSummary]) -> String {
    if fields.is_empty() {
        return "none".into();
    }
    fields
        .iter()
        .map(|field| format!("{}:{:?}", field.key, field.rule))
        .collect::<Vec<_>>()
        .join("  ")
}

fn human_kind(kind: PatchbaySubjectKind) -> &'static str {
    match kind {
        PatchbaySubjectKind::Gear => "GEAR",
        PatchbaySubjectKind::Composition => "COMPOSED GEAR",
        PatchbaySubjectKind::FaceInput => "FORM INPUT",
        PatchbaySubjectKind::FaceOutput => "FORM OUTPUT",
        PatchbaySubjectKind::PortInput => "INPUT PORT",
        PatchbaySubjectKind::PortOutput => "OUTPUT PORT",
        PatchbaySubjectKind::Cord => "CORD",
    }
}

fn meaningful_action(kind: PatchbaySubjectKind) -> &'static str {
    match kind {
        PatchbaySubjectKind::Gear => "ACTION Ctrl+J / Ctrl+Enter",
        PatchbaySubjectKind::Composition => "ACTION  Enter: open Back",
        PatchbaySubjectKind::Cord => "ACTION  Delete: remove Cord",
        PatchbaySubjectKind::PortOutput | PatchbaySubjectKind::FaceInput => {
            "ACTION  drag to compatible input"
        }
        PatchbaySubjectKind::PortInput | PatchbaySubjectKind::FaceOutput => {
            "ACTION  inspect exact typed Port"
        }
    }
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
        origin + Point::new(0, 18),
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
        text(
            target,
            origin + Point::new(0, row as i32 * 18),
            &chunk.iter().collect::<String>(),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_inspection_projects_every_authoritative_port_and_configuration_rule() {
        let palette = patchbay_model::GearPalette::standard().unwrap();
        let entry = palette
            .find(&conduit_core::KindId::from("flow/gate"))
            .unwrap();
        let ports = format!(
            "{} {}",
            port_contracts(&entry.inputs),
            port_contracts(&entry.outputs)
        );
        for port in entry.inputs.iter().chain(&entry.outputs) {
            assert!(ports.contains(port.port_id.as_str()));
            assert!(ports.contains(port.value_kind.as_str()));
        }
        let configuration = configuration_contracts(&entry.configuration);
        for field in &entry.configuration {
            assert!(configuration.contains(&field.key));
            assert!(configuration.contains(&format!("{:?}", field.rule)));
        }
    }
}

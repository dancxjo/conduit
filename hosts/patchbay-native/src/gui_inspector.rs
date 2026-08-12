//! Human-first selected-subject inspection with explicit exact-evidence disclosure.

use crate::{gui::LifecycleContext, interaction_status::InteractionStatus};

pub(super) struct InspectorView<'a> {
    pub(super) selected: Option<&'a str>,
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

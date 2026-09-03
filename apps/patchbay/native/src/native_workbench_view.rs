//! Native Body and History manifestation over the shared workbench models.

use crate::{
    canvas::SoftwareCanvas,
    gui::{HitTarget, HEADER_HEIGHT},
    gui_hit::{GuiAction, HitShape},
    gui_primitives::{frame_rect, icon_label, text, PixelRect},
    icon::Icon,
    native_workbench::NativeBodyWorkbench,
    workbench_chrome::draw_workbench_tabs,
};
use conduit_presentation::{PresentationAspect, PresentationDepth, PresentationPlace};
use embedded_graphics::prelude::Point;
use patchbay_model::{CurrentBodyLifecycle, PHOSPHOR_THEME};

const CARD_LEFT: i32 = 28;
const CARD_TOP: i32 = HEADER_HEIGHT + 22;
const LINE_HEIGHT: i32 = 22;

pub(super) fn workbench_lines(workbench: &NativeBodyWorkbench, linear: bool) -> Vec<String> {
    if workbench.is_history() {
        return history_lines(workbench, linear);
    }
    if workbench.is_program() {
        return program_lines(workbench);
    }
    let frame = workbench.frame();
    let mut lines = vec![
        format!("BODY / STRUCTURE · {:?}", workbench.depth()),
        frame.friendly_name.clone(),
        format!("PROGRAM {}", frame.program.label),
        frame.status_line.clone(),
        frame.placement_line.into(),
        format!(
            "LATEST evidence-sequence={} kind={:?}",
            frame.latest_evidence.sequence, frame.latest_evidence.kind
        ),
        format!(
            "ACTION {:?} · unavailable until an authoritative attached-Body command boundary is supplied",
            frame.salient_action
        ),
    ];
    if workbench.depth() >= PresentationDepth::Detail || linear {
        lines.push(format!("BODY-ID {}", frame.body_id.as_str()));
        lines.push(format!(
            "SOURCE {} · CHECKED {}",
            frame.program.source_document_id.as_str(),
            frame.program.checked_form_id.as_str()
        ));
        for host in &frame.current_hosts {
            lines.push(format!(
                "PART {} · HOST {} · BOOT {} · OFFER {} · OBSERVATION {}",
                host.part_id.as_str(),
                host.host_id.as_str(),
                host.boot_id.as_str(),
                host.offer_generation.0,
                host.observation_sequence
            ));
        }
    }
    if workbench.depth() == PresentationDepth::Exact || linear {
        lines.push(format!("PATCHBAY-READER {:?}", frame.patchbay_reader));
        lines.push(format!(
            "LATEST-SIGN {}",
            frame.latest_evidence.sign_id.as_str()
        ));
    }
    lines
}

fn program_lines(workbench: &NativeBodyWorkbench) -> Vec<String> {
    let frame = workbench.frame();
    vec![
        format!("PROGRAM / STRUCTURE · {:?}", workbench.depth()),
        frame.program.label.clone(),
        "The exact Form source is not attached to this reader.".into(),
        "Open the matching canonical Form to use the native Program canvas.".into(),
        format!(
            "SOURCE {} · CHECKED {}",
            frame.program.source_document_id.as_str(),
            frame.program.checked_form_id.as_str()
        ),
    ]
}

fn history_lines(workbench: &NativeBodyWorkbench, linear: bool) -> Vec<String> {
    let history = workbench.history();
    let mut lines = vec![format!("BODY / SIGNS · HISTORY · {:?}", workbench.depth())];
    for (index, entry) in history.entries.iter().enumerate() {
        let marker = if index == workbench.history_focus() {
            ">"
        } else {
            " "
        };
        lines.push(format!(
            "{marker} EVIDENCE {} · {}",
            match entry.moment {
                patchbay_model::BodyHistoryMoment::EvidenceSequence(sequence) => sequence,
            },
            entry.title
        ));
        if !linear {
            lines.push(format!("  {}", entry.narrative));
        }
        if (workbench.depth() >= PresentationDepth::Detail && index == workbench.history_focus())
            || linear
        {
            lines.push(format!("  SIGN {}", entry.inspect.sign_id.as_str()));
        }
        if (workbench.depth() == PresentationDepth::Exact && index == workbench.history_focus())
            || linear
        {
            lines.push(format!("  {}", entry.linear));
        }
    }
    lines
}

pub(super) fn draw_native_workbench(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    workbench: &NativeBodyWorkbench,
) -> Vec<HitTarget> {
    let mut canvas = SoftwareCanvas::new(pixels, width, height);
    let theme = &PHOSPHOR_THEME;
    let width_i32 = i32::try_from(width).unwrap_or(i32::MAX);
    let mut targets = Vec::with_capacity(3 + workbench.history().entries.len());
    draw_workbench_tabs(&mut canvas, width_i32, workbench, theme, &mut targets);

    if workbench.place() == PresentationPlace::Body
        && workbench.aspect() == PresentationAspect::Structure
    {
        draw_body(&mut canvas, workbench, theme);
    } else if workbench.is_history() {
        draw_history(&mut canvas, workbench, theme, &mut targets);
    } else {
        draw_program_unavailable(&mut canvas, workbench, theme);
    }
    targets
}

pub(super) fn draw_program_tabs(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    workbench: &NativeBodyWorkbench,
    targets: &mut Vec<HitTarget>,
) {
    let mut canvas = SoftwareCanvas::new(pixels, width, height);
    draw_workbench_tabs(
        &mut canvas,
        i32::try_from(width).unwrap_or(i32::MAX),
        workbench,
        &PHOSPHOR_THEME,
        targets,
    );
}

fn draw_body(
    canvas: &mut SoftwareCanvas<'_>,
    workbench: &NativeBodyWorkbench,
    theme: &patchbay_model::PatchbayTheme,
) {
    let frame = workbench.frame();
    icon_label(
        canvas,
        Icon::Body,
        Point::new(CARD_LEFT, CARD_TOP),
        &frame.friendly_name,
        theme.emphasis,
    );
    text(
        canvas,
        Point::new(CARD_LEFT, CARD_TOP + 32),
        &frame.program.label,
        theme.text_primary,
    );
    text(
        canvas,
        Point::new(CARD_LEFT, CARD_TOP + 58),
        &frame.status_line,
        theme.text_secondary,
    );
    text(
        canvas,
        Point::new(CARD_LEFT, CARD_TOP + 84),
        frame.placement_line,
        theme.text_secondary,
    );
    let lifecycle = match &frame.lifecycle {
        CurrentBodyLifecycle::Lulled => "LULLED",
        CurrentBodyLifecycle::Awake { .. } => "AWAKE",
    };
    let action = PixelRect {
        x: CARD_LEFT,
        y: CARD_TOP + 118,
        width: 210,
        height: 38,
    };
    frame_rect(canvas, action, theme.structure_secondary, 1);
    text(
        canvas,
        Point::new(action.x + 10, action.y + 13),
        &format!("{lifecycle} · {:?}", frame.salient_action),
        theme.text_secondary,
    );
    for (index, line) in workbench_lines(workbench, false)
        .into_iter()
        .skip(7)
        .enumerate()
    {
        text(
            canvas,
            Point::new(CARD_LEFT, CARD_TOP + 180 + index as i32 * LINE_HEIGHT),
            &line,
            theme.text_secondary,
        );
    }
}

fn draw_history(
    canvas: &mut SoftwareCanvas<'_>,
    workbench: &NativeBodyWorkbench,
    theme: &patchbay_model::PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    icon_label(
        canvas,
        Icon::Sign,
        Point::new(CARD_LEFT, CARD_TOP),
        "BODY BIOGRAPHY",
        theme.emphasis,
    );
    let mut y = CARD_TOP + 38;
    for (index, entry) in workbench.history().entries.iter().enumerate() {
        let bounds = PixelRect {
            x: CARD_LEFT,
            y,
            width: 940,
            height: 64,
        };
        let selected = index == workbench.history_focus();
        frame_rect(
            canvas,
            bounds,
            if selected {
                theme.focus
            } else {
                theme.structure_secondary
            },
            if selected { 2 } else { 1 },
        );
        let patchbay_model::BodyHistoryMoment::EvidenceSequence(sequence) = entry.moment;
        text(
            canvas,
            Point::new(bounds.x + 10, bounds.y + 10),
            &format!("EVIDENCE {sequence} · {}", entry.title),
            if selected {
                theme.focus
            } else {
                theme.text_primary
            },
        );
        text(
            canvas,
            Point::new(bounds.x + 10, bounds.y + 35),
            &entry.narrative,
            theme.text_secondary,
        );
        targets.push(HitTarget {
            action: GuiAction::InspectHistoryEntry(index),
            shape: HitShape::Rect(bounds),
        });
        y += 76;
    }
    if workbench.depth() >= PresentationDepth::Detail {
        for (index, line) in history_lines(workbench, false)
            .into_iter()
            .filter(|line| line.starts_with("  SIGN") || line.starts_with("  BODY_BIOGRAPHY"))
            .enumerate()
        {
            text(
                canvas,
                Point::new(CARD_LEFT, y + 8 + index as i32 * LINE_HEIGHT),
                &line,
                theme.text_secondary,
            );
        }
    }
}

fn draw_program_unavailable(
    canvas: &mut SoftwareCanvas<'_>,
    workbench: &NativeBodyWorkbench,
    theme: &patchbay_model::PatchbayTheme,
) {
    icon_label(
        canvas,
        Icon::Form,
        Point::new(CARD_LEFT, CARD_TOP),
        &workbench.frame().program.label,
        theme.emphasis,
    );
    text(
        canvas,
        Point::new(CARD_LEFT, CARD_TOP + 38),
        "The exact Form source is not attached to this reader.",
        theme.text_secondary,
    );
    text(
        canvas,
        Point::new(CARD_LEFT, CARD_TOP + 64),
        "Open the matching canonical Form to use the native Program canvas.",
        theme.text_secondary,
    );
}

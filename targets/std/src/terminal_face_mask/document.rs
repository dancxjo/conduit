//! One shared semantic reading order, wrapped without dropping hidden content.
use super::*;
use conduit_presentation::{
    plan_face_utterances, readable_finite_text_choices, render_linear_presentation,
    FaceUtteranceProvenance,
};

pub(super) struct Row {
    pub text: String,
    pub clause: Option<usize>,
    pub control: Option<TerminalControl>,
}

pub(super) fn prepare(
    face: &Presentation,
    width: usize,
) -> Result<(Vec<Row>, Vec<Row>), TerminalError> {
    let plan = plan_face_utterances(face).map_err(|_| TerminalError::DocumentPressure)?;
    let mut rows = Vec::new();
    for (index, clause) in plan.clauses.iter().enumerate() {
        let names = match &clause.provenance {
            FaceUtteranceProvenance::Action(p) => Some((p.identity(), None)),
            FaceUtteranceProvenance::ActionArgument(p) => {
                Some((p.action_identity(), Some(p.argument_name())))
            }
            _ => None,
        };
        let control = names
            .map(|(name, argument)| {
                let action = face
                    .actions
                    .iter()
                    .position(|a| &a.identity == name)
                    .ok_or(TerminalError::InvalidFace)?;
                let argument = argument
                    .map(|name| {
                        face.actions[action]
                            .arguments
                            .iter()
                            .position(|a| &a.name == name)
                            .ok_or(TerminalError::InvalidFace)
                    })
                    .transpose()?;
                Ok::<_, TerminalError>(TerminalControl { action, argument })
            })
            .transpose()?;
        let readable_choice = control.and_then(|control| {
            let argument = face.actions[control.action]
                .arguments
                .get(control.argument?)?;
            let choices = readable_finite_text_choices(&argument.contract)?;
            Some(format!(
                "For {}, choose {}: {}.",
                face.actions[control.action].name,
                argument.value_name,
                choices.join(", ")
            ))
        });
        append(
            &mut rows,
            readable_choice.as_deref().unwrap_or(&clause.text),
            width,
            Some(index),
            control,
        )?;
    }
    let linear = render_linear_presentation(face).map_err(|_| TerminalError::DocumentPressure)?;
    let mut inspect = Vec::new();
    for line in linear.lines {
        append(&mut inspect, &line, width, None, None)?;
    }
    let bytes: usize = rows.iter().chain(&inspect).map(|r| r.text.len()).sum();
    if bytes > MAX_TERMINAL_DOCUMENT_BYTES {
        return Err(TerminalError::DocumentPressure);
    }
    Ok((rows, inspect))
}

/// Escape terminal controls, including bidi/format controls. Semantic bytes
/// remain intact in the Face; this is a display-only safe fallback.
fn safe(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        if ch.is_control() || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            result.extend(ch.escape_default());
        } else {
            result.push(ch);
        }
    }
    result
}
fn append(
    rows: &mut Vec<Row>,
    text: &str,
    width: usize,
    clause: Option<usize>,
    control: Option<TerminalControl>,
) -> Result<(), TerminalError> {
    // Conservative two cells per non-ASCII scalar keeps wide glyphs inside the
    // admitted terminal extent without dropping their actual wording.
    let mut line = String::new();
    let mut cells = 0;
    for ch in safe(text).chars() {
        let size = if ch.is_ascii() { 1 } else { 2 };
        if cells + size > width {
            rows.push(Row {
                text: std::mem::take(&mut line),
                clause,
                control,
            });
            cells = 0;
        }
        line.push(ch);
        cells += size;
        if rows.len() >= MAX_TERMINAL_ROWS {
            return Err(TerminalError::DocumentPressure);
        }
    }
    rows.push(Row {
        text: line,
        clause,
        control,
    });
    Ok(())
}
fn clipped(text: &str, cells: usize) -> String {
    let mut used = 0;
    safe(text)
        .chars()
        .take_while(|ch| {
            used += if ch.is_ascii() { 1 } else { 2 };
            used <= cells
        })
        .collect()
}
pub(super) fn frame(mask: &TerminalFaceMask) -> String {
    let title = mask
        .face
        .subjects
        .first()
        .map_or("Conduit", |s| s.name.as_str());
    let mut frame = format!(
        "\x1b[2J\x1b[H\x1b[1m{}\x1b[0m\r\n{}\r\n",
        clipped(title, mask.columns),
        "─".repeat(mask.columns)
    );
    let rows = mask.current_rows();
    for (index, row) in rows
        .iter()
        .enumerate()
        .skip(mask.top)
        .take(mask.page_rows())
    {
        let marker = if row.control.is_some() && row.control == mask.focus {
            "> "
        } else if index == mask.reading {
            "· "
        } else {
            "  "
        };
        frame.push_str(marker);
        frame.push_str(&row.text);
        frame.push_str("\r\n");
    }
    for _ in rows.len().saturating_sub(mask.top).min(mask.page_rows())..mask.page_rows() {
        frame.push_str("\r\n");
    }
    let preview = mask
        .focus
        .and_then(|c| c.argument.map(|i| (c.action, i)))
        .and_then(|(a, i)| mask.drafts[a][i].as_ref());
    let status = preview
        .map(|v| format!("Draft: {}", String::from_utf8_lossy(v)))
        .unwrap_or_else(|| {
            format!(
                "{} · lines {}–{} / {}",
                if mask.inspect { "Inspect" } else { "Face" },
                mask.top + 1,
                (mask.top + mask.page_rows()).min(rows.len()),
                rows.len()
            )
        });
    frame.push_str(&clipped(&status, mask.columns));
    frame.push_str("\r\n");
    frame.push_str(&clipped(
        if mask.interaction_admitted {
            "Tab focus · ↑↓ read · PgUp/PgDn · F2 inspect · Enter apply · Esc cancel"
        } else {
            "Read only · ↑↓ read · PgUp/PgDn · F2 inspect · Esc close"
        },
        mask.columns,
    ));
    frame
}

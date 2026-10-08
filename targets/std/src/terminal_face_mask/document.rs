//! Face disclosure shapes the compact terminal view; Inspect retains the exact
//! linear projection and its complete provenance.
use super::*;
use conduit_presentation::{
    plan_face_utterances, readable_finite_text_choices, render_linear_presentation,
    FaceUtterancePlan, FaceUtteranceProvenance, PresentationDisclosureLevel,
    PresentationPropertyValue, PresentationRole,
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
    let mut rows = if face.disclosures.is_empty() {
        Vec::new()
    } else {
        primary_rows(face, &plan, width)?
    };
    if face.disclosures.is_empty() {
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

fn primary_rows(
    face: &Presentation,
    plan: &FaceUtterancePlan,
    width: usize,
) -> Result<Vec<Row>, TerminalError> {
    let level = |subject: &str| {
        face.disclosures
            .iter()
            .find(|item| item.subject == subject)
            .map(|item| item.level)
    };
    let has_context = face.disclosures.iter().any(|item| {
        matches!(
            item.level,
            PresentationDisclosureLevel::Context | PresentationDisclosureLevel::Primary
        ) && face.subjects.iter().any(|subject| {
            subject.identity == item.subject
                && matches!(
                    subject.role,
                    PresentationRole::Collection | PresentationRole::Document
                )
        })
    });
    let mut rows = Vec::new();
    for subject in face.subjects.iter().filter(|subject| {
        matches!(
            level(&subject.identity),
            Some(PresentationDisclosureLevel::Context | PresentationDisclosureLevel::Primary)
        ) && matches!(
            subject.role,
            PresentationRole::Collection | PresentationRole::Document
        )
    }) {
        append(
            &mut rows,
            &subject.name,
            width,
            subject_clause(plan, &subject.identity),
            None,
        )?;
    }
    for subject in face.subjects.iter().filter(|subject| {
        level(&subject.identity) == Some(PresentationDisclosureLevel::Primary)
            && subject.role == PresentationRole::Status
            && !face
                .text
                .iter()
                .any(|wording| wording.subject == subject.identity)
    }) {
        append(
            &mut rows,
            &subject.name,
            width,
            subject_clause(plan, &subject.identity),
            None,
        )?;
    }
    for (index, wording) in face.text.iter().enumerate() {
        if !matches!(
            level(&wording.subject),
            Some(PresentationDisclosureLevel::Context | PresentationDisclosureLevel::Primary)
        ) {
            continue;
        }
        let subject = face
            .subjects
            .iter()
            .find(|subject| subject.identity == wording.subject)
            .ok_or(TerminalError::InvalidFace)?;
        if (has_context && subject.role == PresentationRole::Body)
            || (wording.text == subject.name
                && matches!(
                    subject.role,
                    PresentationRole::Collection
                        | PresentationRole::Document
                        | PresentationRole::Status
                        | PresentationRole::Item
                ))
        {
            continue;
        }
        let clause = plan.clauses.iter().position(|clause| {
            matches!(&clause.provenance,
            FaceUtteranceProvenance::Text(source) if *source.index() as usize == index)
        });
        append(&mut rows, &wording.text, width, clause, None)?;
    }
    let mut items = face
        .subjects
        .iter()
        .enumerate()
        .filter(|(_, subject)| {
            level(&subject.identity) == Some(PresentationDisclosureLevel::Primary)
                && subject.role == PresentationRole::Item
        })
        .collect::<Vec<_>>();
    items.sort_by_key(|(index, subject)| {
        (
            face.properties
                .iter()
                .find_map(|property| {
                    (property.subject == subject.identity && property.name == "order")
                        .then_some(&property.value)
                        .and_then(|value| match value {
                            PresentationPropertyValue::Count(order) => Some(*order),
                            _ => None,
                        })
                })
                .unwrap_or(*index as u64),
            *index,
        )
    });
    for (number, (_, subject)) in items.iter().enumerate() {
        let complete = face.properties.iter().any(|property| {
            property.subject == subject.identity
                && property.name == "complete"
                && property.value == PresentationPropertyValue::Flag(true)
        });
        append(
            &mut rows,
            &format!(
                "{}. [{}] {}",
                number + 1,
                if complete { "x" } else { " " },
                subject.name
            ),
            width,
            subject_clause(plan, &subject.identity),
            None,
        )?;
    }
    if rows.is_empty() {
        if let Some(subject) = face.subjects.iter().find(|subject| {
            matches!(
                level(&subject.identity),
                None | Some(PresentationDisclosureLevel::Primary)
            ) && (!has_context || subject.role != PresentationRole::Body)
        }) {
            append(
                &mut rows,
                &subject.name,
                width,
                subject_clause(plan, &subject.identity),
                None,
            )?;
        }
    }
    for (index, action) in face.actions.iter().enumerate() {
        let target_level = level(&action.target);
        if !action.availability.is_available()
            || action.disclosure != PresentationDisclosureLevel::CurrentAction
            || !(matches!(
                target_level,
                Some(PresentationDisclosureLevel::Context | PresentationDisclosureLevel::Primary)
            ) || (!has_context && target_level.is_none()))
        {
            continue;
        }
        let target = face
            .subjects
            .iter()
            .find(|subject| subject.identity == action.target)
            .ok_or(TerminalError::InvalidFace)?;
        let label = if target.role == PresentationRole::Item {
            format!("{} · {}", action.name, target.name)
        } else {
            action.name.clone()
        };
        let clause = plan.clauses.iter().position(|clause| {
            matches!(&clause.provenance,
            FaceUtteranceProvenance::Action(source) if source.identity() == &action.identity)
        });
        append(
            &mut rows,
            &label,
            width,
            clause,
            Some(TerminalControl {
                action: index,
                argument: None,
            }),
        )?;
        for (argument_index, argument) in action.arguments.iter().enumerate() {
            let hint = readable_finite_text_choices(&argument.contract).map_or_else(
                || argument.value_name.clone(),
                |choices| format!("{}: {}", argument.value_name, choices.join(", ")),
            );
            let argument_clause = plan.clauses.iter().position(|clause| matches!(&clause.provenance,
                FaceUtteranceProvenance::ActionArgument(source) if source.action_identity() == &action.identity && source.argument_name() == &argument.name));
            append(
                &mut rows,
                &hint,
                width,
                argument_clause,
                Some(TerminalControl {
                    action: index,
                    argument: Some(argument_index),
                }),
            )?;
        }
    }
    Ok(rows)
}

fn subject_clause(plan: &FaceUtterancePlan, identity: &str) -> Option<usize> {
    plan.clauses.iter().position(|clause| {
        matches!(&clause.provenance,
        FaceUtteranceProvenance::Subject(source) if source.identity() == identity)
    })
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
        .iter()
        .find(|subject| {
            mask.face.disclosures.iter().any(|item| {
                item.subject == subject.identity
                    && matches!(
                        item.level,
                        PresentationDisclosureLevel::Context | PresentationDisclosureLevel::Primary
                    )
            }) && matches!(
                subject.role,
                PresentationRole::Collection | PresentationRole::Document
            )
        })
        .or_else(|| mask.face.subjects.first())
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

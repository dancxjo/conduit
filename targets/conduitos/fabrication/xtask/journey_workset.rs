//! Exact Form switching inside one ordinary native Body Play.
mod causality;
mod input;
pub(super) use causality::validate as validate_causality;
#[cfg(test)]
mod tests;

use conduitos::native_workset::{self, NativeForm};
use serde::Serialize;
use serde_json::Value;

use super::ConduitosError;
pub(super) use input::exercise;

#[derive(Serialize)]
pub(super) struct WorksetProof {
    pub forms: Vec<FormProof>,
    pub switches: usize,
    pub input_count: u64,
    pub held_release_crossed_selection: bool,
    pub memory_cleared_and_edited_again: bool,
    pub resident_tour_ran: bool,
    pub patchbay_edit_requested: bool,
    pub patchbay_presenters_replanned: bool,
}

#[derive(Serialize)]
pub(super) struct FormProof {
    title: &'static str,
    source_document_id: String,
    checked_form_id: String,
    expanded_form_id: String,
    final_result: String,
}

#[derive(Clone, Copy)]
struct Expected {
    form: NativeForm,
    count: u64,
    result: Option<&'static str>,
}

fn expected() -> Vec<Expected> {
    use NativeForm::{KeyboardCanvas as Canvas, MemoryLantern as Memory, Patchbay, Tour};
    let mut records = (0..=10u64)
        .map(|count| Expected {
            form: Canvas,
            count,
            result: (count != 0).then(|| &"HELLO"[..count.div_ceil(2) as usize]),
        })
        .collect::<Vec<_>>();
    // Tab changes the foreground without consuming a Form input. Each ordinary
    // press/release is accepted exactly once; the held X release belongs to
    // Canvas even though Memory is foreground by then.
    records.extend([
        Expected {
            form: Patchbay,
            count: 10,
            result: None,
        },
        Expected {
            form: Patchbay,
            count: 11,
            result: None,
        },
        Expected {
            form: Patchbay,
            count: 12,
            result: None,
        },
        Expected {
            form: Patchbay,
            count: 13,
            result: None,
        },
        Expected {
            form: Tour,
            count: 13,
            result: None,
        },
        Expected {
            form: Tour,
            count: 14,
            result: None,
        },
    ]);
    for (form, count, result) in [
        (Memory, 10, None),
        (Memory, 11, Some("o")),
        (Memory, 12, Some("o")),
        (Memory, 13, Some("on")),
        (Memory, 14, Some("on")),
        (Memory, 15, Some("one")),
        (Memory, 16, Some("one")),
        (Canvas, 16, Some("HELLO")),
        (Canvas, 17, Some("HELLOX")),
        (Patchbay, 17, None),
        (Tour, 17, None),
        (Memory, 17, Some("one")),
        (Memory, 18, Some("one")),
        (Memory, 19, Some("on")),
        (Memory, 20, Some("on")),
        (Memory, 21, Some("o")),
        (Memory, 22, Some("o")),
        (Memory, 23, Some("")),
        (Memory, 24, Some("")),
        (Memory, 25, Some("h")),
        (Memory, 26, Some("h")),
        (Memory, 27, Some("hi")),
        (Memory, 28, Some("hi")),
        (Canvas, 28, Some("HELLOX")),
        (Canvas, 29, Some("HELLOXY")),
        (Canvas, 30, Some("HELLOXY")),
        (Patchbay, 30, None),
        (Tour, 30, None),
        (Memory, 30, Some("hi")),
        (Canvas, 30, Some("HELLOXY")),
    ] {
        records.push(Expected {
            form,
            count: count + 4,
            result,
        });
    }
    records
}

fn identity(form: NativeForm) -> Result<FormProof, ConduitosError> {
    let checked = native_workset::checked(form)
        .map_err(|error| ConduitosError::refusal("product-workset-catalog", error.as_str()))?;
    Ok(FormProof {
        title: form.title(),
        source_document_id: checked.source_document_id.as_str().into(),
        checked_form_id: checked.checked_form_id.as_str().into(),
        expanded_form_id: checked.expanded_form_id.as_str().into(),
        final_result: String::new(),
    })
}

fn matches(record: &Value, expected: Expected, form: &FormProof) -> bool {
    record["status"] == "quiescent-awaiting-input"
        && record["input_count"] == expected.count
        && match expected.result {
            Some(result) => record["result"].as_str() == Some(result),
            None => record.get("result") == Some(&Value::Null),
        }
        && record["source_document_id"] == form.source_document_id
        && record["checked_form_id"] == form.checked_form_id
        && record["expanded_form_id"] == form.expanded_form_id
        && record["result_omitted_bytes"] == 0
        && record["kernel_sign_gap"].is_null()
}

pub(super) fn validate(records: &[Value]) -> Result<(&Value, WorksetProof), ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "product-journey-workset-invalid",
            "four exact resident Forms must retain independent state and one Body through Presenter replanning, switching, inspection, held release, empty editing, and Lull",
        )
    };
    let mut canvas = identity(NativeForm::KeyboardCanvas)?;
    let mut memory = identity(NativeForm::MemoryLantern)?;
    let tour = identity(NativeForm::Tour)?;
    let patchbay = identity(NativeForm::Patchbay)?;
    let quiescent = records
        .iter()
        .filter(|record| record["status"] == "quiescent-awaiting-input")
        .collect::<Vec<_>>();
    let expected = expected();
    if quiescent.len() > expected.len() {
        return Err(refusal());
    }
    let mut expected_index = 0;
    for record in &quiescent {
        while expected_index < expected.len() {
            let candidate = expected[expected_index];
            let form = match candidate.form {
                NativeForm::KeyboardCanvas => &canvas,
                NativeForm::MemoryLantern => &memory,
                NativeForm::Tour => &tour,
                NativeForm::Patchbay => &patchbay,
            };
            if matches(record, candidate, form) {
                break;
            }
            expected_index += 1;
        }
        if expected_index == expected.len() {
            return Err(refusal());
        }
        let matched_index = expected_index;
        expected_index += 1;
        for field in ["body_id", "wake_id"] {
            if quiescent[0][field].as_str().is_none_or(str::is_empty)
                || record[field] != quiescent[0][field]
            {
                return Err(refusal());
            }
        }
        let realization_basis = if matched_index < 14 {
            quiescent[0]
        } else {
            quiescent
                .iter()
                .copied()
                .find(|candidate| matches(candidate, expected[14], &patchbay))
                .ok_or_else(refusal)?
        };
        for field in ["plan_id", "active_play_id"] {
            if realization_basis[field].as_str().is_none_or(str::is_empty)
                || record[field] != realization_basis[field]
            {
                return Err(refusal());
            }
        }
    }
    let replanned = quiescent
        .iter()
        .copied()
        .find(|record| matches(record, expected[14], &patchbay))
        .ok_or_else(refusal)?;
    if ["plan_id", "active_play_id"]
        .iter()
        .any(|field| replanned[field] == quiescent[0][field])
    {
        return Err(refusal());
    }
    // Presentation service may coalesce adjacent accepted inputs. Preserve the
    // critical semantic checkpoints instead of requiring one frame per event.
    for index in [0, 10, 11, 12, 13, 20, 22, 23, 24, 26, 34, 37, 39, 40, 43] {
        let checkpoint = expected[index];
        let form = match checkpoint.form {
            NativeForm::KeyboardCanvas => &canvas,
            NativeForm::MemoryLantern => &memory,
            NativeForm::Tour => &tour,
            NativeForm::Patchbay => &patchbay,
        };
        if !quiescent
            .iter()
            .any(|record| matches(record, checkpoint, form))
        {
            return Err(refusal());
        }
    }
    // Zero-Body arrival has no lifecycle Form. Once explicit selection births
    // the Body, every projection must name an exact reviewed identity tuple.
    for record in records {
        if record["status"] == "world" {
            if ["source_document_id", "checked_form_id", "expanded_form_id"]
                .iter()
                .any(|field| !record[field].is_null())
            {
                return Err(refusal());
            }
            continue;
        }
        let reviewed = [&canvas, &memory, &tour, &patchbay].iter().any(|form| {
            record["source_document_id"] == form.source_document_id
                && record["checked_form_id"] == form.checked_form_id
                && record["expanded_form_id"] == form.expanded_form_id
        });
        if !reviewed {
            return Err(refusal());
        }
    }
    for status in ["stopped", "lulled"] {
        let record = records
            .iter()
            .find(|record| record["status"] == status)
            .ok_or_else(refusal)?;
        if ["body_id", "wake_id", "plan_id", "active_play_id"]
            .iter()
            .any(|field| record[field] != replanned[field])
            || record["body_id"] != quiescent[0]["body_id"]
            || record["wake_id"] != quiescent[0]["wake_id"]
            || record["input_count"] != 34
            || record["result"] != "HELLOXY"
        {
            return Err(refusal());
        }
    }
    canvas.final_result = "HELLOXY".into();
    memory.final_result = "hi".into();
    Ok((
        quiescent
            .iter()
            .copied()
            .find(|record| matches(record, expected[10], &canvas))
            .ok_or_else(refusal)?,
        WorksetProof {
            forms: vec![canvas, memory, tour, patchbay],
            switches: quiescent
                .windows(2)
                .filter(|pair| pair[0]["checked_form_id"] != pair[1]["checked_form_id"])
                .count(),
            input_count: 34,
            held_release_crossed_selection: true,
            memory_cleared_and_edited_again: true,
            resident_tour_ran: true,
            patchbay_edit_requested: true,
            patchbay_presenters_replanned: true,
        },
    ))
}

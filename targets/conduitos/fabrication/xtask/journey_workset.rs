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
    use NativeForm::{KeyboardCanvas as Canvas, MemoryLantern as Memory};
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
        (Memory, 30, Some("hi")),
        (Canvas, 30, Some("HELLOXY")),
    ] {
        records.push(Expected {
            form,
            count,
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
    record["status"] == "playing"
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
            "two exact Forms must retain independent results and one Body/Plan/Play through switching, held release, empty editing, and Lull",
        )
    };
    let mut canvas = identity(NativeForm::KeyboardCanvas)?;
    let mut memory = identity(NativeForm::MemoryLantern)?;
    let playing = records
        .iter()
        .filter(|record| record["status"] == "playing")
        .collect::<Vec<_>>();
    let expected = expected();
    if playing.len() != expected.len() {
        return Err(refusal());
    }
    for (record, expected) in playing.iter().zip(&expected) {
        let form = match expected.form {
            NativeForm::KeyboardCanvas => &canvas,
            NativeForm::MemoryLantern => &memory,
        };
        if !matches(record, *expected, form) {
            return Err(refusal());
        }
        for field in ["body_id", "wake_id", "plan_id", "active_play_id"] {
            if playing[0][field].as_str().is_none_or(str::is_empty)
                || record[field] != playing[0][field]
            {
                return Err(refusal());
            }
        }
    }
    // Pre-birth records retain the initial Form identity. After birth each
    // selection must still name an exact reviewed source/checked/expanded tuple.
    for record in records {
        let reviewed = [&canvas, &memory].iter().any(|form| {
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
            .any(|field| record[field] != playing[0][field])
            || record["input_count"] != 30
            || record["result"] != "HELLOXY"
        {
            return Err(refusal());
        }
    }
    canvas.final_result = "HELLOXY".into();
    memory.final_result = "hi".into();
    Ok((
        playing[10],
        WorksetProof {
            forms: vec![canvas, memory],
            switches: playing
                .windows(2)
                .filter(|pair| pair[0]["checked_form_id"] != pair[1]["checked_form_id"])
                .count(),
            input_count: 30,
            held_release_crossed_selection: true,
            memory_cleared_and_edited_again: true,
        },
    ))
}

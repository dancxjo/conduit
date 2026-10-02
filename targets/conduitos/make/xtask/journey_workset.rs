//! Exact plot switching inside one ordinary native Body Play.
mod causality;
mod input;
pub(super) use causality::validate as validate_causality;
#[cfg(test)]
mod tests;

use conduitos::native_workset::{self, NativePlot};
use serde::Serialize;
use serde_json::Value;

use super::ConduitosError;
pub(super) use input::exercise;

#[derive(Serialize)]
pub(super) struct WorksetProof {
    pub plots: Vec<PlotProof>,
    pub switches: usize,
    pub input_count: u64,
    pub held_release_crossed_selection: bool,
    pub memory_cleared_and_edited_again: bool,
    pub resident_tour_ran: bool,
    pub patchbay_edit_requested: bool,
    pub patchbay_presenters_replanned: bool,
}

#[derive(Serialize)]
pub(super) struct PlotProof {
    title: &'static str,
    source_document_id: String,
    checked_plot_id: String,
    expanded_plot_id: String,
    final_result: String,
}

#[derive(Clone, Copy)]
struct Expected {
    plot: NativePlot,
    count: u64,
    result: Option<&'static str>,
}

fn expected() -> Vec<Expected> {
    use NativePlot::{KeyboardCanvas as Canvas, MemoryLantern as Memory, Patchbay, Tour};
    let mut records = (0..=10u64)
        .map(|count| Expected {
            plot: Canvas,
            count,
            result: (count != 0).then(|| &"HELLO"[..count.div_ceil(2) as usize]),
        })
        .collect::<Vec<_>>();
    // Tab changes the foreground without consuming a plot input. Each ordinary
    // press/release is accepted exactly once; the held X release belongs to
    // Canvas even though Memory is foreground by then.
    records.extend([
        Expected {
            plot: Patchbay,
            count: 10,
            result: None,
        },
        Expected {
            plot: Patchbay,
            count: 11,
            result: None,
        },
        Expected {
            plot: Patchbay,
            count: 12,
            result: None,
        },
        Expected {
            plot: Patchbay,
            count: 13,
            result: None,
        },
        Expected {
            plot: Tour,
            count: 13,
            result: None,
        },
        Expected {
            plot: Tour,
            count: 14,
            result: None,
        },
    ]);
    for (plot, count, result) in [
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
            plot,
            count: count + 4,
            result,
        });
    }
    records
}

fn identity(plot: NativePlot) -> Result<PlotProof, ConduitosError> {
    let checked = native_workset::checked(plot)
        .map_err(|error| ConduitosError::refusal("product-workset-catalog", error.as_str()))?;
    Ok(PlotProof {
        title: plot.title(),
        source_document_id: checked.source_document_id.as_str().into(),
        checked_plot_id: checked.checked_plot_id.as_str().into(),
        expanded_plot_id: checked.expanded_plot_id.as_str().into(),
        final_result: String::new(),
    })
}

fn matches(record: &Value, expected: Expected, plot: &PlotProof) -> bool {
    record["status"] == "quiescent-awaiting-input"
        && record["input_count"] == expected.count
        && match expected.result {
            Some(result) => record["result"].as_str() == Some(result),
            None => record.get("result") == Some(&Value::Null),
        }
        && (plot.source_document_id.is_empty()
            || record["source_document_id"] == plot.source_document_id)
        && (plot.checked_plot_id.is_empty() || record["checked_plot_id"] == plot.checked_plot_id)
        && (plot.expanded_plot_id.is_empty() || record["expanded_plot_id"] == plot.expanded_plot_id)
        && record["result_omitted_bytes"] == 0
        && record["kernel_sign_gap"].is_null()
}

fn observe_identity(plot: &mut PlotProof, record: &Value) -> Result<(), ConduitosError> {
    let observed = [
        ("source_document_id", &mut plot.source_document_id),
        ("checked_plot_id", &mut plot.checked_plot_id),
        ("expanded_plot_id", &mut plot.expanded_plot_id),
    ];
    for (field, retained) in observed {
        let value = record[field]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ConduitosError::refusal("product-journey-workset-invalid", field))?;
        if retained.is_empty() {
            *retained = value.into();
        } else if retained != value {
            return Err(ConduitosError::refusal(
                "product-journey-workset-invalid",
                format!("{field} changed within one resident Plot"),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate(records: &[Value]) -> Result<(&Value, WorksetProof), ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "product-journey-workset-invalid",
            "four exact resident Plots must retain independent state and one body through Presenter replanning, switching, inspection, held release, empty editing, and Lull",
        )
    };
    // The reviewed identities classify otherwise identical zero-input
    // projections; observe_identity below then verifies that the producer
    // emitted that exact tuple consistently for each resident Plot.
    let mut canvas = identity(NativePlot::KeyboardCanvas)?;
    let mut memory = identity(NativePlot::MemoryLantern)?;
    let mut tour = identity(NativePlot::Tour)?;
    let mut patchbay = identity(NativePlot::Patchbay)?;
    // The containing ConduitOS proof may have exercised an earlier wake
    // before the product journey. The final wake is introduced explicitly by
    // the journey and owns every checkpoint through its Lull terminal.
    let journey_records = records
        .iter()
        .rposition(|record| record["status"] == "awake")
        .map_or(records, |index| &records[index..]);
    let mut saw_pre_input_baseline = false;
    let quiescent = journey_records
        .iter()
        .filter(|record| record["status"] == "quiescent-awaiting-input")
        .filter(|record| record["workload_revision"] == 1)
        .filter(|record| {
            if record["input_count"].as_u64() != Some(0) {
                return true;
            }
            // Home may switch among installed plots before the first plot input.
            // Keep one lifecycle baseline; those launcher-only projections do
            // not become additional workset execution checkpoints.
            let keep = !saw_pre_input_baseline;
            saw_pre_input_baseline = true;
            keep
        })
        .collect::<Vec<_>>();
    let expected = expected();
    let mut expected_index = 0;
    for (record_index, record) in quiescent.iter().enumerate() {
        // Connectivity and inspection can re-project the already completed
        // terminal foreground state without accepting another Plot input.
        if expected_index == expected.len() {
            break;
        }
        let current_plot = expected[expected_index].plot;
        let plot = match current_plot {
            NativePlot::KeyboardCanvas => &canvas,
            NativePlot::MemoryLantern => &memory,
            NativePlot::Tour => &tour,
            NativePlot::Patchbay => &patchbay,
        };
        let same_plot = record["source_document_id"] == plot.source_document_id
            && record["checked_plot_id"] == plot.checked_plot_id
            && record["expanded_plot_id"] == plot.expanded_plot_id;
        if !same_plot {
            continue;
        }
        let block_end = (expected_index..expected.len())
            .find(|index| expected[*index].plot != current_plot)
            .unwrap_or(expected.len());
        let matched_index = (expected_index..block_end).find(|candidate_index| {
            let candidate = expected[*candidate_index];
            matches(record, candidate, plot)
        });
        let Some(matched_index) = matched_index else {
            // Switching can project an older retained state before the next
            // action. A current-block count with the wrong value, or an exact
            // duplicate revision, is malformed evidence rather than a switch.
            let current_count = expected[expected_index..block_end]
                .iter()
                .any(|candidate| record["input_count"] == candidate.count);
            let duplicate_revision =
                record_index > 0 && record["revision"] == quiescent[record_index - 1]["revision"];
            if current_count || duplicate_revision {
                return Err(refusal());
            }
            continue;
        };
        expected_index = matched_index + 1;
        match expected[matched_index].plot {
            NativePlot::KeyboardCanvas => observe_identity(&mut canvas, record)?,
            NativePlot::MemoryLantern => observe_identity(&mut memory, record)?,
            NativePlot::Tour => observe_identity(&mut tour, record)?,
            NativePlot::Patchbay => observe_identity(&mut patchbay, record)?,
        }
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
        let plot = match checkpoint.plot {
            NativePlot::KeyboardCanvas => &canvas,
            NativePlot::MemoryLantern => &memory,
            NativePlot::Tour => &tour,
            NativePlot::Patchbay => &patchbay,
        };
        if !quiescent
            .iter()
            .any(|record| matches(record, checkpoint, plot))
        {
            return Err(refusal());
        }
    }
    // Zero-Body arrival has no lifecycle Plot. Once explicit selection births
    // the body, every projection must name an exact reviewed identity tuple.
    for record in journey_records {
        if record["status"] == "world" {
            if ["source_document_id", "checked_plot_id", "expanded_plot_id"]
                .iter()
                .any(|field| !record[field].is_null())
            {
                return Err(refusal());
            }
            continue;
        }
        let reviewed = [&canvas, &memory, &tour, &patchbay].iter().any(|plot| {
            record["source_document_id"] == plot.source_document_id
                && record["checked_plot_id"] == plot.checked_plot_id
                && record["expanded_plot_id"] == plot.expanded_plot_id
        });
        if !reviewed {
            return Err(refusal());
        }
    }
    for status in ["stopped", "lulled"] {
        let record = journey_records
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
            plots: vec![canvas, memory, tour, patchbay],
            switches: quiescent
                .windows(2)
                .filter(|pair| pair[0]["checked_plot_id"] != pair[1]["checked_plot_id"])
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

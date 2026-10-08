//! Ordinary graphical input, exact Body identity, and retained domain cost.
use super::super::{ConduitosError, journey_input, journey_records, qmp};
use serde_json::Value;
use std::{fs, os::unix::net::UnixStream, path::Path, process::Child};

fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("journey-keyboard-domain", detail)
}
fn latest(serial: &Path) -> Result<Value, ConduitosError> {
    let bytes = fs::read(serial).map_err(|error| refusal(error.to_string()))?;
    journey_records::decode(journey_input::complete_records(&bytes)?)?
        .pop()
        .ok_or_else(|| refusal("missing journey record"))
}

pub(super) fn exercise(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
) -> Result<Value, ConduitosError> {
    exercise_plot(
        serial,
        child,
        stream,
        reader,
        conduitos::native_workset::NativePlot::KeyboardCanvas,
        &[("a", "A")],
    )
}

pub(super) fn exercise_editor(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
) -> Result<Value, ConduitosError> {
    exercise_plot(
        serial,
        child,
        stream,
        reader,
        conduitos::native_workset::NativePlot::MemoryLantern,
        &[
            ("a", "a"),
            ("b", "ab"),
            ("backspace", "a"),
            ("backspace", ""),
            ("c", "c"),
        ],
    )
}

pub(super) fn verify_sibling(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
) -> Result<(), ConduitosError> {
    exercise_plot(
        serial,
        child,
        stream,
        reader,
        conduitos::native_workset::NativePlot::KeyboardCanvas,
        &[],
    )?;
    if latest(serial)?["result"] != "A" {
        return Err(refusal(
            "retained editor changed its sibling's presentation",
        ));
    }
    Ok(())
}

fn exercise_plot(
    serial: &Path,
    child: &mut Child,
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    plot: conduitos::native_workset::NativePlot,
    inputs: &[(&str, &str)],
) -> Result<Value, ConduitosError> {
    let expected =
        conduitos::native_workset::resident(plot).map_err(|error| refusal(format!("{error:?}")))?;
    let initial = latest(serial)?;
    for _ in 0..conduitos::native_workset::NATIVE_PLOT_CAPACITY {
        let current = latest(serial)?;
        if current["checked_plot_id"] == expected.checked_plot_id.as_str() {
            break;
        }
        let revision = current["revision"]
            .as_u64()
            .ok_or_else(|| refusal("missing revision"))?;
        journey_input::key_pair(stream, reader, "tab", "journey-select-keyboard-canvas")?;
        journey_input::wait_for_record(
            serial,
            child,
            "journey-keyboard-selection-timeout",
            "keyboard canvas selection",
            |text| {
                Ok(journey_records::decode(text)?.last().is_some_and(|record| {
                    record["revision"]
                        .as_u64()
                        .is_some_and(|next| next > revision)
                }))
            },
        )?;
    }
    let selected = latest(serial)?;
    if selected["checked_plot_id"] != expected.checked_plot_id.as_str()
        || selected["source_document_id"] != expected.source_document_id.as_str()
        || selected["active_play_id"] != initial["active_play_id"]
        || selected["plan_id"] != initial["plan_id"]
    {
        return Err(refusal(
            "selection replaced the admitted Body Play or did not select the checked keyboard Source",
        ));
    }
    for (key, expected_result) in inputs {
        let revision = latest(serial)?["revision"]
            .as_u64()
            .ok_or_else(|| refusal("missing revision"))?;
        journey_input::key_pair(stream, reader, key, "journey-protected-text-input")?;
        journey_input::wait_for_record(
            serial,
            child,
            "journey-protected-text-timeout",
            expected_result,
            |text| {
                Ok(journey_records::decode(text)?.last().is_some_and(|record| {
                    record["result"] == *expected_result
                        && record["status"] == "quiescent-awaiting-input"
                        && record["checked_plot_id"] == expected.checked_plot_id.as_str()
                        && record["revision"]
                            .as_u64()
                            .is_some_and(|next| next > revision)
                        && record["active_play_id"] == initial["active_play_id"]
                        && record["plan_id"] == initial["plan_id"]
                }))
            },
        )?;
    }
    Ok(selected)
}

pub(super) fn validate_cost(
    serial: &str,
    identity: &Value,
    entries: u64,
    effects: u64,
) -> Result<Value, ConduitosError> {
    let costs = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_DOMAIN_COST "))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|error| refusal(error.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let matching = costs
        .into_iter()
        .filter(|cost| {
            cost["plan_id"] == identity["plan_id"]
                && cost["play_id"] == identity["active_play_id"]
                && cost["checked_plot_id"] == identity["checked_plot_id"]
                && cost["partition_plan_id"] == identity["partition_plan_id"]
                && cost["fixture"] == false
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(refusal(
            "missing or duplicate exact Body partition domain cost",
        ));
    }
    let cost = matching.into_iter().next().unwrap();
    if cost["entries"]
        .as_u64()
        .is_none_or(|actual| actual != entries)
        || cost["base_gate_transitions"] != effects
        || cost["state"] != "Revoked(PlayCancelled)"
        || cost["partition_plan_id"].as_str().is_none_or(str::is_empty)
        || cost["teardown_zeroed_bytes"] != 151552
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(refusal(
            "Body domain did not execute and retire its bounded keyboard effect",
        ));
    }
    Ok(cost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cost_requires_one_exact_current_partition_and_retirement() {
        let identity = json!({"plan_id":"body-plan", "active_play_id":"play", "checked_plot_id":"editor", "partition_plan_id":"editor-partition"});
        let cost = json!({"plan_id":"body-plan", "play_id":"play", "checked_plot_id":"editor",
            "partition_plan_id":"editor-partition", "fixture":false, "entries":21,
            "base_gate_transitions":5, "state":"Revoked(PlayCancelled)", "teardown_zeroed_bytes":151552,
            "dma_isolation":false, "driver_isolation":false});
        let line = |cost: &Value| format!("CONDUIT_DOMAIN_COST {cost}\n");
        assert!(validate_cost(&line(&cost), &identity, 21, 5).is_ok());
        assert!(validate_cost(&line(&cost).repeat(2), &identity, 21, 5).is_err());
        for (field, replacement) in [
            ("play_id", "old-play"),
            ("checked_plot_id", "sibling"),
            ("partition_plan_id", "foreign-partition"),
            ("state", "Ready"),
        ] {
            let mut invalid = cost.clone();
            invalid[field] = json!(replacement);
            assert!(
                validate_cost(&line(&invalid), &identity, 21, 5).is_err(),
                "{field}"
            );
        }
        assert!(validate_cost(&line(&cost), &identity, 5, 1).is_err());
    }
}

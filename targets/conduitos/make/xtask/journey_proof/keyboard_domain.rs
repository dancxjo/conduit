//! Ordinary graphical input, exact Body identity, and retained domain cost.
use super::super::{journey_input, journey_records, qmp, ConduitosError};
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
    let expected = conduitos::keyboard_text_plan::checked_plot_identity()
        .map_err(|error| refusal(error.as_str()))?;
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
    journey_input::key_pair(stream, reader, "a", "journey-protected-keyboard-input")?;
    journey_input::wait_for_record(
        serial,
        child,
        "journey-protected-keyboard-timeout",
        "protected uppercase A",
        |text| {
            Ok(journey_records::decode(text)?.last().is_some_and(|record| {
                record["result"] == "A"
                    && record["status"] == "quiescent-awaiting-input"
                    && record["active_play_id"] == initial["active_play_id"]
            }))
        },
    )?;
    Ok(selected)
}

pub(super) fn validate_cost(serial: &str, identity: &Value) -> Result<Value, ConduitosError> {
    let costs = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_DOMAIN_COST "))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|error| refusal(error.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let cost = costs
        .into_iter()
        .find(|cost| {
            cost["plan_id"] == identity["plan_id"]
                && cost["play_id"] == identity["active_play_id"]
                && cost["fixture"] == false
        })
        .ok_or_else(|| refusal("missing exact Body domain cost"))?;
    if cost["entries"].as_u64().is_none_or(|entries| entries != 5)
        || cost["base_gate_transitions"] != 1
        || cost["teardown_zeroed_bytes"] != 118784
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(refusal(
            "Body domain did not execute and retire its bounded keyboard effect",
        ));
    }
    Ok(cost)
}

//! Validate bounded Root progress before deriving stage attribution.
use super::ConduitosError;
use conduitos::native_observation::Phase;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const PREFIX: &str = "CONDUIT_NATIVE_PROGRESS ";
const REFUSAL: &str = "CONDUIT_NATIVE_PROGRESS_REFUSAL ";
#[derive(Default, Serialize)]
struct Aggregate {
    count: u32,
    refused: u32,
    aggregate_nested_ticks: u64,
    maximum_ticks: u64,
}

pub(super) fn parse(serial: &str, host: &str, boot: &str) -> Result<Value, ConduitosError> {
    let mut records = Vec::new();
    let mut stack: Vec<Value> = Vec::new();
    let mut next_span = 1;
    let mut previous = None;
    let mut custody = 0;
    let mut phases = BTreeMap::<String, Aggregate>::new();
    for line in serial.split_inclusive('\n') {
        if line.starts_with(REFUSAL) {
            return Err(refusal("guest progress observer refused"));
        }
        if let Some(text) = line.strip_prefix("CONDUIT_NATIVE_FACE ") {
            if text.len() > 96 * 1024 || !line.ends_with('\n') {
                return Err(refusal("incomplete Face custody"));
            }
            let face: Value =
                serde_json::from_str(text).map_err(|_| refusal("invalid Face custody"))?;
            if face["show"]["host_id"] != host || face["show"]["boot_id"] != boot {
                return Err(refusal("progress crosses Host/Boot custody"));
            }
            custody += 1;
        }
        let Some(text) = line.strip_prefix(PREFIX) else {
            continue;
        };
        if !line.ends_with('\n') || line.len() > 512 || records.len() == 2048 {
            return Err(refusal("incomplete or oversized progress family"));
        }
        let record: Value =
            serde_json::from_str(text).map_err(|_| refusal("invalid progress JSON"))?;
        let ticks = record["ticks"]
            .as_u64()
            .ok_or_else(|| refusal("invalid raw counter"))?;
        let id = record["span"]
            .as_u64()
            .ok_or_else(|| refusal("invalid span"))?;
        let parent = record["parent"]
            .as_u64()
            .ok_or_else(|| refusal("invalid parent"))?;
        let phase = record["phase"]
            .as_str()
            .ok_or_else(|| refusal("missing phase"))?;
        let context = record["context"]
            .as_str()
            .ok_or_else(|| refusal("missing context"))?;
        if record.as_object().is_none_or(|fields| fields.len() != 11)
            || record["schema"] != "conduit.conduitos/native-progress@1"
            || record["scope"] != "native-root-boot"
            || record["sequence"].as_u64() != Some(records.len() as u64)
            || record["clock"] != "x86-tsc-lfence"
            || record.get("frequency_hz") != Some(&Value::Null)
            || previous.is_some_and(|prior| ticks < prior)
            || !Phase::ALL.iter().any(|kind| kind.as_str() == phase)
            || context.is_empty()
            || context.len() > 64
            || !context
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_/.:".contains(&b))
        {
            return Err(refusal(
                "progress identity, sequence, clock or context mismatch",
            ));
        }
        match record["state"].as_str() {
            Some("started") => {
                if id != next_span
                    || id > 1024
                    || parent != stack.last().map_or(0, |row| row["span"].as_u64().unwrap())
                {
                    return Err(refusal("progress parent or span reuse"));
                }
                next_span += 1;
                stack.push(record.clone());
            }
            Some("finished" | "refused") => {
                let start = stack
                    .pop()
                    .ok_or_else(|| refusal("progress end without start"))?;
                if start["span"] != record["span"]
                    || start["parent"] != record["parent"]
                    || start["phase"] != record["phase"]
                    || start["context"] != record["context"]
                {
                    return Err(refusal("progress scope ended out of order"));
                }
                let elapsed = ticks
                    .checked_sub(start["ticks"].as_u64().unwrap())
                    .ok_or_else(|| refusal("progress counter reversed"))?;
                let aggregate = phases.entry(phase.into()).or_default();
                aggregate.count += 1;
                aggregate.refused += u32::from(record["state"] == "refused");
                aggregate.aggregate_nested_ticks = aggregate
                    .aggregate_nested_ticks
                    .checked_add(elapsed)
                    .ok_or_else(|| refusal("progress aggregate overflow"))?;
                aggregate.maximum_ticks = aggregate.maximum_ticks.max(elapsed);
            }
            _ => return Err(refusal("abandoned or unknown progress outcome")),
        }
        previous = Some(ticks);
        records.push(record);
    }
    if records.is_empty() || !stack.is_empty() || custody == 0 || host.is_empty() || boot.is_empty()
    {
        return Err(refusal("progress lacks a complete Boot-bound family"));
    }
    let network_exercised = phases.contains_key(Phase::NetworkSetup.as_str());
    Ok(
        json!({"schema":"conduit.conduitos/native-progress-proof@1","status":"complete",
        "host_id":host,"boot_id":boot,"clock":"x86-tsc-lfence","frequency_hz":null,
        "record_limit":2048,"span_limit":1024,"records":records,"phases":phases,
        "optional_network_exercised":network_exercised,
        "interpretation":"One synchronous Root Boot stream, bound to the same journey's independently validated native Face/Show Host/Boot custody. Progress events are diagnostic observations, not Signs. Raw counters have no calibrated frequency; intervals include observer overhead, can nest, and are not additive or exclusive CPU time. Refused phases retain operation failures without changing their semantic outcome."}),
    )
}

pub(super) fn require_product_phases(progress: &Value) -> Result<(), ConduitosError> {
    for phase in Phase::ALL {
        // An unprovisioned local product has no owner network attempt.
        if phase != Phase::NetworkSetup
            && progress["phases"][phase.as_str()]["count"]
                .as_u64()
                .unwrap_or(0)
                == 0
        {
            return Err(refusal(
                "ordinary graphical journey missed a required progress phase",
            ));
        }
    }
    Ok(())
}
fn refusal(detail: &'static str) -> ConduitosError {
    ConduitosError::refusal("native-progress-invalid", detail)
}
#[cfg(test)]
mod tests;

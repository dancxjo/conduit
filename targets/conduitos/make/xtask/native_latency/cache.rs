//! Cache observations are separate from the unchanged required progress stream.
use super::{CacheMode, ConduitosError};
use conduitos::native_workset::{candidate_inventory, NATIVE_CANDIDATE_CAPACITY};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
const PREFIX: &str = "CONDUIT_NATIVE_CACHE ";
const SCHEMA: &str = "conduit.conduitos/native-cache@1";

pub(super) fn parse(
    serial: &str,
    mode: CacheMode,
    host: &str,
    boot: &str,
) -> Result<Value, ConduitosError> {
    let mut records = Vec::new();
    let mut totals = [0u64; 7];
    let mut custody = 0;
    let mut shared_sources = BTreeSet::new();
    let mut shared_expansions = BTreeSet::new();
    let mut fallbacks = 0u64;
    let limits = match mode {
        CacheMode::Retained => [NATIVE_CANDIDATE_CAPACITY as u64, 1, 8192],
        CacheMode::ZeroRetention => [0, 0, 0],
    };
    let mut retained = [0u64; 2];
    for line in serial.split_inclusive('\n') {
        if line.starts_with("CONDUIT_NATIVE_CACHE_REFUSAL ") {
            return Err(refusal("guest cache observer refused"));
        }
        if let Some(text) = line.strip_prefix("CONDUIT_NATIVE_FACE ") {
            if !line.ends_with('\n') || text.len() > 96 * 1024 {
                return Err(refusal("incomplete cache custody"));
            }
            let face: Value =
                serde_json::from_str(text).map_err(|_| refusal("invalid cache custody"))?;
            if face["show"]["host_id"] != host || face["show"]["boot_id"] != boot {
                return Err(refusal("cache crosses Host/Boot custody"));
            }
            custody += 1;
        }
        let Some(text) = line.strip_prefix(PREFIX) else {
            continue;
        };
        if !line.ends_with('\n') || line.len() > 768 || records.len() == 512 {
            return Err(refusal("incomplete or oversized cache family"));
        }
        let record: Value =
            serde_json::from_str(text).map_err(|_| refusal("invalid cache JSON"))?;
        let delta = numbers::<7>(&record["delta"])?;
        let held = numbers::<2>(&record["retained"])?;
        let plot = record["plot"]
            .as_str()
            .ok_or_else(|| refusal("missing plot"))?;
        let source = candidate_inventory()
            .into_iter()
            .find(|p| p.name() == plot)
            .ok_or_else(|| refusal("foreign native plot"))?;
        let digest = format!("{:x}", Sha256::digest(source.source().as_bytes()));
        let accepted = record["accepted"]
            .as_bool()
            .ok_or_else(|| refusal("missing cache outcome"))?;
        if record.as_object().is_none_or(|m| m.len() != 11)
            || record["schema"] != SCHEMA
            || record["scope"] != "native-root-boot"
            || record["sequence"].as_u64() != Some(records.len() as u64)
            || record["mode"] != mode.as_str()
            || numbers::<3>(&record["limits"])? != limits
            || record["source_sha256"] != digest
            || delta.iter().any(|v| *v > 1)
            || delta[0] + delta[1] != 1
            || delta[2] + delta[3] > 1
            || (accepted && delta[2] + delta[3] != 1)
            || delta[6] > delta[0]
            || held[0] > limits[0]
            || held[1] > limits[0] * limits[1]
        {
            return Err(refusal(
                "cache identity, bounds, sequence or outcome mismatch",
            ));
        }
        match record["path"].as_str() {
            Some("shared") => {
                if delta[1] > 0 && !shared_sources.contains(plot)
                    || delta[3] > 0 && !shared_expansions.contains(plot)
                {
                    return Err(refusal("hit without retained current Source/expansion"));
                }
                if held[0] < retained[0] || held[1] < retained[1] {
                    return Err(refusal("shared retention reset"));
                }
                retained = held;
                if accepted && mode == CacheMode::Retained {
                    shared_sources.insert(plot.to_owned());
                    shared_expansions.insert(plot.to_owned());
                }
            }
            Some("private-contention") => {
                if delta[1] != 0 || delta[3] != 0 || held[0] > 1 || held[1] > 1 {
                    return Err(refusal("private fallback claimed shared retention hits"));
                }
                fallbacks = fallbacks
                    .checked_add(1)
                    .ok_or_else(|| refusal("fallback counter overflow"))?;
            }
            _ => return Err(refusal("unknown cache path")),
        }
        if mode == CacheMode::ZeroRetention
            && (held != [0, 0]
                || delta[1] != 0
                || delta[3] != 0
                || delta[4] != 0
                || delta[5] != 0
                || accepted && delta[6] != 1)
        {
            return Err(refusal(
                "zero retention retained entries, hits or omitted cold work",
            ));
        }
        if delta[4] != 0 || delta[5] != 0 {
            return Err(refusal("finite native inventory unexpectedly evicted"));
        }
        for (total, value) in totals.iter_mut().zip(delta) {
            *total = total
                .checked_add(value)
                .ok_or_else(|| refusal("cache counter overflow"))?;
        }
        records.push(record);
    }
    if records.is_empty()
        || custody == 0
        || totals[0] == 0
        || totals[2] == 0
        || mode == CacheMode::Retained && (totals[1] == 0 || totals[3] == 0)
    {
        return Err(refusal(
            "missing cache work, hits or current Host/Boot custody",
        ));
    }
    Ok(
        json!({"schema":SCHEMA,"mode":mode.as_str(),"host_id":host,"boot_id":boot,
        "limits":limits,"totals":{"checks":totals[0],"check_hits":totals[1],"expansions":totals[2],"expansion_hits":totals[3],"source_evictions":totals[4],"expansion_evictions":totals[5],"unretained_checks":totals[6]},
        "private_contention_calls":fallbacks,"final_shared_retained":retained,
        "interpretation":"Per-call canonical stats deltas; checked sums include independent private cold fallbacks. Retained counts are per selected store, not global memory accounting. Both modes share the same installed catalogs. No Plan or Host admission is cached.","records":records}),
    )
}
fn numbers<const N: usize>(value: &Value) -> Result<[u64; N], ConduitosError> {
    let values = value
        .as_array()
        .filter(|v| v.len() == N)
        .ok_or_else(|| refusal("invalid cache counters"))?;
    let mut result = [0; N];
    for (out, value) in result.iter_mut().zip(values) {
        *out = value
            .as_u64()
            .ok_or_else(|| refusal("invalid cache counter"))?;
    }
    Ok(result)
}
fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("native-cache-observation", detail)
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;

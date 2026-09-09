//! Repeated real keyboard input in one ordinary native Body/Plan/Play.

use std::{os::unix::net::UnixStream, path::Path, process::Child};

use serde_json::Value;

use super::{journey_input, journey_records, qmp, ConduitosError};

pub(super) fn type_hello(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    serial: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    for (index, key) in ["h", "e", "l", "l", "o"].into_iter().enumerate() {
        journey_input::key_pair(stream, reader, key, "standing-input")?;
        let expected = &"HELLO"[..index + 1];
        journey_input::wait_for_record(
            serial,
            child,
            "product-journey-standing-input-timeout",
            expected,
            |text| {
                Ok(journey_records::decode(text)?.iter().any(|record| {
                    record["status"] == "playing"
                        && record["result"] == expected
                        && record["input_count"] == ((index + 1) * 2) as u64
                }))
            },
        )?;
    }
    Ok(())
}

pub(super) fn validate(records: &[Value]) -> Result<&Value, ConduitosError> {
    let refuse = || {
        ConduitosError::refusal(
            "product-journey-standing-play-invalid",
            "HELLO must traverse one unchanged Body, Plan, and Play before explicit Stop",
        )
    };
    let playing = records
        .iter()
        .filter(|record| record["status"] == "playing")
        .collect::<Vec<_>>();
    let first = playing.first().ok_or_else(refuse)?;
    let result = *playing.last().ok_or_else(refuse)?;
    for identity in ["body_id", "plan_id", "active_play_id"] {
        if first[identity].as_str().is_none_or(str::is_empty)
            || playing
                .iter()
                .any(|record| record[identity] != first[identity])
        {
            return Err(refuse());
        }
    }
    // Every down/up event is accepted exactly once. A release retains the same
    // value, so checking both count and prefix catches lost or duplicated keys.
    for count in 0..=10u64 {
        let matches = playing
            .iter()
            .filter(|record| record["input_count"] == count);
        if matches.count() != 1 {
            return Err(refuse());
        }
        let record = playing
            .iter()
            .find(|record| record["input_count"] == count)
            .unwrap();
        let expected = &"HELLO"[..count.div_ceil(2) as usize];
        if (count == 0 && !record["result"].is_null())
            || (count > 0 && record["result"] != expected)
            || record["result_omitted_bytes"] != 0
            || !record["kernel_sign_gap"].is_null()
        {
            return Err(refuse());
        }
    }
    if playing.len() != 11 || result["result"] != "HELLO" {
        return Err(refuse());
    }
    let stopped = records
        .iter()
        .find(|record| record["status"] == "stopped")
        .ok_or_else(refuse)?;
    if stopped["result"] != result["result"]
        || stopped["input_count"] != 10
        || stopped["body_id"] != result["body_id"]
    {
        return Err(refuse());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> Vec<Value> {
        let mut records = (0..=10u64).map(|count| serde_json::json!({
            "status": "playing", "body_id": "body", "plan_id": "plan", "active_play_id": "play",
            "input_count": count, "result": if count == 0 { Value::Null } else { Value::from(&"HELLO"[..count.div_ceil(2) as usize]) },
            "result_omitted_bytes": 0, "kernel_sign_gap": null
        })).collect::<Vec<_>>();
        let mut stopped = records.last().unwrap().clone();
        stopped["status"] = "stopped".into();
        records.push(stopped);
        records
    }

    #[test]
    fn standing_proof_rejects_restarts_lost_releases_and_implicit_completion() {
        let valid = records();
        assert_eq!(validate(&valid).unwrap()["result"], "HELLO");
        for field in ["body_id", "plan_id", "active_play_id"] {
            let mut changed = valid.clone();
            changed[5][field] = "replacement".into();
            assert!(validate(&changed).is_err());
        }
        let mut missing = valid.clone();
        missing.remove(6);
        assert!(validate(&missing).is_err());
        let mut duplicate = valid.clone();
        duplicate.insert(6, duplicate[6].clone());
        assert!(validate(&duplicate).is_err());
        let mut completed = valid.clone();
        completed[10]["status"] = "result-visible".into();
        assert!(validate(&completed).is_err());
        let mut control_leak = valid;
        control_leak[1]["result"] = "\nH".into();
        assert!(validate(&control_leak).is_err());
    }
}

//! Validate guest counter observations before correlating them with frame custody.
use super::ConduitosError;
use serde_json::{json, Value};

pub(super) fn parse(serial: &str) -> Result<Value, ConduitosError> {
    let refuse = || {
        ConduitosError::refusal(
            "native-timing-invalid",
            "incomplete, reversed or malformed observation family",
        )
    };
    let mut records = Vec::new();
    let mut previous_end = None;
    for line in serial.lines() {
        if line.starts_with("CONDUIT_NATIVE_TIMING_REFUSAL ") {
            return Err(refuse());
        }
        let Some(text) = line.strip_prefix("CONDUIT_NATIVE_TIMING ") else {
            for prefix in [
                "CONDUIT_NATIVE_FACE ",
                "CONDUIT_CRECHE_CHANGE ",
                "CONDUIT_CRECHE_FOCUS ",
                "CONDUIT_NATIVE_POINTER ",
                "CONDUIT_NATIVE_BIRTH ",
                "CONDUIT_NATIVE_ACTION ",
                "CONDUIT_NATIVE_READING ",
                "CONDUIT_NATIVE_JOURNEY_TIMING ",
            ] {
                let Some(text) = line.strip_prefix(prefix) else {
                    continue;
                };
                let record: Value = serde_json::from_str(text).map_err(|_| refuse())?;
                if let Some(clock) = record.get("native_observer") {
                    let ticks = clock["ticks"].as_u64().ok_or_else(refuse)?;
                    if records.is_empty()
                        || clock["clock"] != "x86-tsc-lfence"
                        || previous_end.is_some_and(|previous| ticks < previous)
                    {
                        return Err(refuse());
                    }
                    previous_end = Some(ticks);
                }
            }
            continue;
        };
        let record: Value = serde_json::from_str(text).map_err(|_| refuse())?;
        let start = record["start_ticks"].as_u64().ok_or_else(refuse)?;
        let end = record["end_ticks"].as_u64().ok_or_else(refuse)?;
        let stage = record["stage"].as_str().ok_or_else(refuse)?;
        if records.len() >= 2048
            || record["schema"] != "conduit.conduitos/native-timing@1"
            || record["sequence"].as_u64() != Some(records.len() as u64)
            || record["clock"] != "x86-tsc-lfence"
            || record.get("frequency_hz") != Some(&Value::Null)
            || record["counter_reversed"] != false
            || end < start
            || previous_end.is_some_and(|previous| end < previous)
            || !matches!(
                stage,
                "boot-entry"
                    | "first-framebuffer-commit"
                    | "plot-build"
                    | "plan-seal"
                    | "workset-prepare"
                    | "kernel-prepare"
                    | "play-start"
                    | "play-input"
                    | "pointer-feedback"
            )
            || (records.is_empty() != (stage == "boot-entry"))
        {
            return Err(refuse());
        }
        previous_end = Some(end);
        records.push(record);
    }
    if records.is_empty() {
        return Ok(Value::Null);
    }
    Ok(
        json!({"clock":"x86-tsc-lfence","units":"ticks","frequency_hz":null,
        "interpretation":"Instrumented Root intervals include nested observer publication and may end on refusal. Outcomes and identities belong to the correlated guest records. First framebuffer commit is not a physical display observation. No ticks-to-time conversion or physical latency claim.",
        "records":records}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(sequence: u64, stage: &str, start: u64, end: u64) -> String {
        format!(
            "CONDUIT_NATIVE_TIMING {}\n",
            json!({"schema":"conduit.conduitos/native-timing@1",
            "sequence":sequence,"stage":stage,"clock":"x86-tsc-lfence","frequency_hz":null,
            "start_ticks":start,"end_ticks":end,"counter_reversed":end < start})
        )
    }
    #[test]
    fn accepts_nested_intervals_without_inventing_clock_frequency() {
        let serial = record(0, "boot-entry", 10, 10)
            + &record(1, "plot-build", 30, 40)
            + &record(2, "workset-prepare", 20, 50);
        let result = parse(&serial).unwrap();
        assert_eq!(result["records"].as_array().unwrap().len(), 3);
        assert!(result["frequency_hz"].is_null());
        assert!(parse("legacy guest\n").unwrap().is_null());
    }
    #[test]
    fn rejects_gaps_reboots_clock_reversal_and_exhaustion() {
        let boot = record(0, "boot-entry", 10, 10);
        for suffix in [
            record(2, "plot-build", 20, 30),
            record(1, "boot-entry", 20, 20),
            record(1, "plot-build", 9, 9),
            record(1, "plot-build", 30, 20),
            "CONDUIT_NATIVE_TIMING_REFUSAL observation-bound\n".into(),
        ] {
            assert!(parse(&(boot.clone() + &suffix)).is_err());
        }
        assert!(parse(&record(0, "plot-build", 10, 20)).is_err());
    }
}

#[cfg(test)]
mod custody_clock_tests {
    use super::*;
    #[test]
    fn custody_receipts_cannot_hide_counter_reversal_between_stage_records() {
        let boot = "CONDUIT_NATIVE_TIMING {\"schema\":\"conduit.conduitos/native-timing@1\",\"sequence\":0,\"stage\":\"boot-entry\",\"clock\":\"x86-tsc-lfence\",\"frequency_hz\":null,\"start_ticks\":10,\"end_ticks\":10,\"counter_reversed\":false}\n";
        for (clock, ticks, valid) in [
            ("x86-tsc-lfence", 11, true),
            ("x86-tsc-lfence", 9, false),
            ("another-clock", 11, false),
        ] {
            let custody = format!(
                "CONDUIT_CRECHE_FOCUS {}\n",
                json!({"native_observer":{"clock":clock,"ticks":ticks}})
            );
            assert_eq!(parse(&(boot.to_owned() + &custody)).is_ok(), valid);
        }
    }
}

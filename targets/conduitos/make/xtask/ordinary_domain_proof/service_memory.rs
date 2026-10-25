//! Reject absent, duplicated or broadened service-backend fixture evidence.
use super::super::ConduitosError;
use serde_json::Value;

const PREFIX: &str = "CONDUIT_SERVICE_MEMORY_PROOF ";
pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let mut lines = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let line = lines
        .next()
        .ok_or_else(|| refusal("service-memory-fixture-absent"))?;
    if lines.next().is_some() {
        return Err(refusal("service-memory-fixture-duplicated"));
    }
    let value: Value =
        serde_json::from_str(line).map_err(|_| refusal("service-memory-fixture-malformed"))?;
    if value["schema"] != "conduit.conduitos/service-memory-fixture@1"
        || value["status"] != "completed"
        || value["proof_class"] != "freestanding-emulator"
        || value["fixture"] != true
        || value["architecture"] != "x86_64"
        || value["privilege"] != "ring3"
        || value["hostile_entries"] != 26
        || value["retained_bytes"] != 8388608
        || value["scratch_bytes"] != 8388608
        || value["restart_boundary_samples_zero"] != true
        || value["production_compositor"] != false
        || value["driver_isolation"] != false
        || value["dma_isolation"] != false
        || value["foreign_application_profile_refused_without_entry"] != true
    {
        return Err(refusal("service-memory-fixture-mismatch"));
    }
    application_receipts(&value["application_rtc"])?;
    Ok(value)
}
/// Exact native observations, never acceptance of a declared cap alone.
fn application_receipts(value: &Value) -> Result<(), ConduitosError> {
    let invalid = || refusal("service-application-rtc-receipt-mismatch");
    if value.as_object().is_none_or(|fields| fields.len() != 8)
        || value["schema"] != "conduit.conduitos/application-rtc-receipt-fixture@1"
    {
        return Err(invalid());
    }
    for field in [
        "prior_immutable",
        "invalid_caps_refused_without_entry",
        "source_wake_once",
        "faults_terminal",
    ] {
        if value[field] != true {
            return Err(invalid());
        }
    }
    for (name, sequence, admitted, consumed, source, result) in [
        ("yield", 1, 1, 0, 0, "yielded"),
        ("fault", 2, 1, 1, 0, "work-exhausted"),
        ("source", 1, 64, 64, 1, "work-exhausted"),
    ] {
        let row = &value[name];
        if row.as_object().is_none_or(|fields| fields.len() != 6)
            || row["instance"].as_u64().is_none_or(|n| n == 0)
            || row["sequence"] != sequence
            || row["admitted_rtc"] != admitted
            || row["consumed_rtc"] != consumed
            || row["result"] != result
            || row["source_interrupts"] != source
            || row["source_interrupts"]
                .as_u64()
                .is_none_or(|n| n > u32::MAX.into())
        {
            return Err(invalid());
        }
    }
    if value["yield"]["instance"] != value["fault"]["instance"]
        || value["source"]["instance"].as_u64() <= value["yield"]["instance"].as_u64()
        || value["source"]["source_interrupts"] != 1
    {
        return Err(invalid());
    }
    Ok(())
}
fn refusal(reason: &'static str) -> ConduitosError {
    ConduitosError::refusal(
        reason,
        "requires the complete x86_64 service-memory fixture; no production compositor, driver or DMA isolation claim",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Value {
        serde_json::json!({"schema":"conduit.conduitos/service-memory-fixture@1", "status":"completed",
        "proof_class":"freestanding-emulator", "fixture":true, "architecture":"x86_64", "privilege":"ring3",
        "hostile_entries":26, "retained_bytes":8388608, "scratch_bytes":8388608,
        "restart_boundary_samples_zero":true, "production_compositor":false, "driver_isolation":false,"dma_isolation":false,
        "foreign_application_profile_refused_without_entry":true,
        "application_rtc": {
            "schema":"conduit.conduitos/application-rtc-receipt-fixture@1",
            "yield":{"instance":7,"sequence":1,"admitted_rtc":1,"consumed_rtc":0,"source_interrupts":0,"result":"yielded"},
            "fault":{"instance":7,"sequence":2,"admitted_rtc":1,"consumed_rtc":1,"source_interrupts":0,"result":"work-exhausted"},
            "source":{"instance":8,"sequence":1,"admitted_rtc":64,"consumed_rtc":64,"source_interrupts":1,"result":"work-exhausted"},
            "prior_immutable":true,"invalid_caps_refused_without_entry":true,"source_wake_once":true,"faults_terminal":true
        }})
    }
    fn transcript(value: &Value) -> String {
        format!("{PREFIX}{value}\n")
    }
    #[test]
    fn requires_one_complete_fixture_and_preserves_its_proof_boundary() {
        let value = sample();
        let text = transcript(&value);
        assert!(validate(&text).is_ok());
        assert!(validate("").is_err());
        assert!(validate(&format!("{text}{text}")).is_err());
        for field in [
            "fixture",
            "restart_boundary_samples_zero",
            "production_compositor",
            "driver_isolation",
            "dma_isolation",
            "foreign_application_profile_refused_without_entry",
        ] {
            let mut changed = value.clone();
            changed[field] = Value::Bool(!changed[field].as_bool().unwrap());
            assert!(validate(&transcript(&changed)).is_err(), "{field}");
        }
        for field in ["hostile_entries", "retained_bytes", "scratch_bytes"] {
            let mut changed = value.clone();
            changed[field] = Value::from(0);
            assert!(validate(&transcript(&changed)).is_err(), "{field}");
        }
    }
    #[test]
    fn rejects_missing_forged_caps_counts_results_and_custody() {
        for path in [
            "/application_rtc",
            "/application_rtc/yield",
            "/application_rtc/fault",
            "/application_rtc/source",
        ] {
            let mut value = sample();
            *value.pointer_mut(path).unwrap() = Value::Null;
            assert!(validate(&transcript(&value)).is_err(), "{path}");
        }
        for (path, replacement) in [
            ("/application_rtc/schema", serde_json::json!("foreign")),
            ("/application_rtc/yield/instance", serde_json::json!(0)),
            ("/application_rtc/fault/instance", serde_json::json!(8)),
            ("/application_rtc/fault/sequence", serde_json::json!(1)),
            ("/application_rtc/yield/admitted_rtc", serde_json::json!(64)),
            ("/application_rtc/yield/consumed_rtc", serde_json::json!(1)),
            ("/application_rtc/fault/consumed_rtc", serde_json::json!(0)),
            (
                "/application_rtc/fault/result",
                serde_json::json!("yielded"),
            ),
            ("/application_rtc/source/instance", serde_json::json!(7)),
            ("/application_rtc/source/sequence", serde_json::json!(2)),
            (
                "/application_rtc/source/admitted_rtc",
                serde_json::json!(65),
            ),
            (
                "/application_rtc/source/consumed_rtc",
                serde_json::json!(65),
            ),
            (
                "/application_rtc/source/source_interrupts",
                serde_json::json!(0),
            ),
            (
                "/application_rtc/source/source_interrupts",
                serde_json::json!(2),
            ),
            (
                "/application_rtc/yield/source_interrupts",
                serde_json::json!(1),
            ),
            (
                "/application_rtc/fault/source_interrupts",
                serde_json::json!(1),
            ),
            (
                "/application_rtc/yield/source_interrupts",
                serde_json::json!(-1),
            ),
            (
                "/application_rtc/fault/source_interrupts",
                serde_json::json!(u64::MAX),
            ),
            ("/application_rtc/prior_immutable", serde_json::json!(false)),
            (
                "/application_rtc/invalid_caps_refused_without_entry",
                serde_json::json!(false),
            ),
            (
                "/application_rtc/source_wake_once",
                serde_json::json!(false),
            ),
            ("/application_rtc/faults_terminal", serde_json::json!(false)),
        ] {
            let mut value = sample();
            *value.pointer_mut(path).unwrap() = replacement;
            assert!(validate(&transcript(&value)).is_err(), "{path}");
        }
        let mut value = sample();
        value["application_rtc"]["grant_increased"] = Value::Bool(true);
        assert!(validate(&transcript(&value)).is_err());
        let mut value = sample();
        value["application_rtc"]["fault"]["retry"] = Value::Bool(true);
        assert!(validate(&transcript(&value)).is_err());
    }
}

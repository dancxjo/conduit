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
    {
        return Err(refusal("service-memory-fixture-mismatch"));
    }
    Ok(value)
}
fn refusal(reason: &'static str) -> ConduitosError {
    ConduitosError::refusal(reason, "requires the complete x86_64 service-memory fixture; no production compositor, driver or DMA isolation claim")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Value {
        serde_json::json!({"schema":"conduit.conduitos/service-memory-fixture@1", "status":"completed",
            "proof_class":"freestanding-emulator", "fixture":true, "architecture":"x86_64", "privilege":"ring3",
            "hostile_entries":26, "retained_bytes":8388608, "scratch_bytes":8388608,
            "restart_boundary_samples_zero":true, "production_compositor":false, "driver_isolation":false,"dma_isolation":false})
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
}

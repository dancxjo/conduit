//! Retain actual ordinary timer product evidence separately from synthetic completions.
use super::ConduitosError;
use serde_json::Value;

const MARKER: &str = "CONDUIT_DOMAIN_TIMER_PRODUCT private-production-kernel physical-duration-120ms exact-capabilities counts-presented-2 timer-wakes-1 cancelled zeroed";

pub(super) fn validate(transcript: &str, architecture: &str) -> Result<Value, ConduitosError> {
    if !transcript.lines().any(|line| line == MARKER)
        || !transcript
            .lines()
            .any(|line| line == "CONDUIT_DOMAIN_TIMER_REPLAY refused-before-provider-access")
    {
        return Err(refusal("actual protected timer product marker absent"));
    }
    let json = super::emitted_line::complete_json_line(transcript, "CONDUIT_DOMAIN_TIMER_COST ")
        .ok_or_else(|| refusal("timer product cost absent"))?;
    let cost: Value = serde_json::from_str(json).map_err(|error| refusal(error.to_string()))?;
    let (reserved, tick_unit) = match architecture {
        "x86_64" => (217088, "tsc"),
        "ia32" => (229376, "tsc"),
        "aarch64" => (225280, "cntvct"),
        "riscv64" => (217088, "time"),
        "loongarch64" => (225280, "rdtime"),
        _ => return Err(refusal("unsupported timer architecture")),
    };
    if cost["schema"] != "conduit.conduitos/domain-cost@1"
        || cost["architecture"] != architecture
        || cost["state"] != "Revoked(PlayCancelled)"
        || cost["plan_id"].as_str().is_none_or(str::is_empty)
        || cost["play_id"].as_str().is_none_or(str::is_empty)
        || cost["domain_id"].as_u64().is_none_or(|id| id == 0)
        || cost["entries"].as_u64().is_none_or(|entries| entries < 8)
        || cost["base_gate_transitions"] != 4
        || cost["scheduler_returns"] != cost["entries"]
        || cost["reserved_bytes"] != reserved
        || cost["teardown_zeroed_bytes"] != reserved
        || cost["setup_copied_bytes"]
            .as_u64()
            .is_none_or(|bytes| bytes == 0 || bytes > 131072)
        || cost["root_metadata_bytes"]
            .as_u64()
            .is_none_or(|bytes| bytes == 0 || bytes > 1048576)
        || cost["tick_unit"] != tick_unit
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(refusal(cost.to_string()));
    }
    Ok(cost)
}

fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("protected-timer-product-unproven", detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cost() -> Value {
        json!({"schema":"conduit.conduitos/domain-cost@1", "architecture":"ia32",
            "state":"Revoked(PlayCancelled)", "plan_id":"plan", "play_id":"play", "domain_id":1,
            "entries":12, "scheduler_returns":12, "base_gate_transitions":4,
            "reserved_bytes":229376, "teardown_zeroed_bytes":229376,
            "setup_copied_bytes":67000, "root_metadata_bytes":8000, "tick_unit":"tsc",
            "dma_isolation":false, "driver_isolation":false})
    }

    fn transcript(cost: &Value) -> String {
        format!(
            "CONDUIT_DOMAIN_TIMER_COST {cost}\n{MARKER}\nCONDUIT_DOMAIN_TIMER_REPLAY refused-before-provider-access\n"
        )
    }

    #[test]
    fn actual_product_requires_its_architecture_complete_gates_and_zeroization() {
        let cost = cost();
        assert_eq!(validate(&transcript(&cost), "ia32").unwrap(), cost);
        for (field, value) in [
            ("architecture", json!("aarch64")),
            ("state", json!("Suspended")),
            ("base_gate_transitions", json!(3)),
            ("entries", json!(0)),
            ("scheduler_returns", json!(11)),
            ("teardown_zeroed_bytes", json!(0)),
            ("setup_copied_bytes", json!(131073)),
            ("tick_unit", json!("time")),
            ("plan_id", Value::Null),
            ("play_id", json!("")),
            ("domain_id", json!(0)),
            ("dma_isolation", json!(true)),
            ("driver_isolation", json!(true)),
        ] {
            let mut altered = cost.clone();
            altered[field] = value;
            assert!(validate(&transcript(&altered), "ia32").is_err(), "{field}");
        }
        assert!(validate(&transcript(&cost), "aarch64").is_err());
        assert!(validate(
            &transcript(&cost).replace(
                "CONDUIT_DOMAIN_TIMER_REPLAY refused-before-provider-access\n",
                ""
            ),
            "ia32"
        )
        .is_err());
        assert!(validate(&format!("CONDUIT_DOMAIN_TIMER_COST {cost}\n"), "ia32").is_err());
        assert!(validate(MARKER, "ia32").is_err());
    }
}

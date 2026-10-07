//! Require the normal product's exact completed domain, not just a ready Sign.
use super::ConduitosError;
use serde_json::Value;

pub(super) fn capture(
    transcript: &str,
    product: &Value,
    architecture: &str,
) -> Result<Value, ConduitosError> {
    let json = super::emitted_line::complete_json_line(transcript, "CONDUIT_DOMAIN_COST ")
        .ok_or_else(|| refusal("normal product domain cost absent"))?;
    let cost: Value = serde_json::from_str(json).map_err(|error| refusal(error.to_string()))?;
    validate(&cost, product, architecture)?;
    Ok(cost)
}

fn validate(cost: &Value, product: &Value, architecture: &str) -> Result<(), ConduitosError> {
    let (reserved, tick_unit) = match architecture {
        "ia32" => (131072, "tsc"),
        "aarch64" => (126976, "cntvct"),
        _ => return Err(refusal("unreviewed product domain architecture")),
    };
    if cost["schema"] != "conduit.conduitos/domain-cost@1"
        || cost["architecture"] != architecture
        || cost["region_id"] != "region/text"
        || product["ordinary_plan_id"]
            .as_str()
            .is_none_or(str::is_empty)
        || product["ordinary_play_id"]
            .as_str()
            .is_none_or(str::is_empty)
        || cost["plan_id"] != product["ordinary_plan_id"]
        || cost["play_id"] != product["ordinary_play_id"]
        || cost["domain_id"].as_u64().is_none_or(|id| id == 0)
        || cost["fixture"] != false
        || cost["state"] != "Revoked(PlayCompleted)"
        || cost["entries"] != 3
        || cost["gate_transitions"] != 3
        || cost["base_gate_transitions"] != 1
        || cost["address_space_switches"] != 6
        || cost["tlb_flushes"] != 6
        || cost["scheduler_returns"] != 3
        || cost["privilege_transitions"]
            .as_u64()
            .is_none_or(|count| count < 6)
        || cost["copied_bytes"] != 64
        || cost["shared_peak_bytes"] != 32
        || cost["shared_page_bytes"] != 4096
        || cost["reserved_bytes"] != reserved
        || cost["teardown_zeroed_bytes"] != cost["reserved_bytes"]
        || cost["root_metadata_bytes"]
            .as_u64()
            .is_none_or(|bytes| bytes == 0 || bytes > 1048576)
        || cost["setup_copied_bytes"]
            .as_u64()
            .is_none_or(|bytes| bytes == 0 || bytes > 65536)
        || cost["setup_ticks"].as_u64().is_none_or(|ticks| ticks == 0)
        || cost["teardown_ticks"]
            .as_u64()
            .is_none_or(|ticks| ticks == 0)
        || cost["tick_unit"] != tick_unit
        || cost["preemptions"] != 0
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(refusal(cost.to_string()));
    }
    Ok(())
}
fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("ordinary-product-domain-receipt-invalid", detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn records() -> (Value, Value) {
        let product = json!({"ordinary_plan_id":"plan", "ordinary_play_id":"play"});
        let cost = json!({
            "schema":"conduit.conduitos/domain-cost@1", "architecture":"ia32",
            "region_id":"region/text", "plan_id":"plan", "play_id":"play",
            "domain_id":1, "fixture":false, "state":"Revoked(PlayCompleted)",
            "entries":3, "gate_transitions":3, "base_gate_transitions":1,
            "address_space_switches":6, "tlb_flushes":6, "scheduler_returns":3,
            "privilege_transitions":6, "copied_bytes":64, "shared_peak_bytes":32,
            "shared_page_bytes":4096, "reserved_bytes":131072,
            "teardown_zeroed_bytes":131072, "root_metadata_bytes":21878,
            "setup_copied_bytes":13194, "setup_ticks":1, "teardown_ticks":1,
            "tick_unit":"tsc", "preemptions":0, "dma_isolation":false,
            "driver_isolation":false
        });
        (cost, product)
    }

    #[test]
    fn completed_product_domain_is_retained() {
        let (cost, product) = records();
        let transcript = format!("HELLO, CONDUITOSCONDUIT_DOMAIN_COST {cost}\n");
        assert_eq!(capture(&transcript, &product, "ia32").unwrap(), cost);
        assert!(capture("CONDUIT_IA32_PRODUCT {}\n", &product, "ia32").is_err());
    }

    #[test]
    fn fixture_stale_owner_missing_gate_and_incomplete_teardown_refuse() {
        let (cost, product) = records();
        for (field, replacement) in [
            ("architecture", json!("x86_64")),
            ("fixture", json!(true)),
            ("plan_id", json!("stale")),
            ("play_id", json!("stale")),
            ("base_gate_transitions", json!(0)),
            ("entries", json!(0)),
            ("state", json!("Suspended")),
            ("teardown_zeroed_bytes", json!(0)),
            ("dma_isolation", json!(true)),
            ("driver_isolation", json!(true)),
        ] {
            let mut altered = cost.clone();
            altered[field] = replacement;
            assert!(validate(&altered, &product, "ia32").is_err(), "{field}");
        }
    }

    #[test]
    fn aarch64_cost_requires_its_own_storage_clock_and_current_owner() {
        let (mut cost, mut product) = records();
        cost["architecture"] = json!("aarch64");
        cost["reserved_bytes"] = json!(126976);
        cost["teardown_zeroed_bytes"] = json!(126976);
        cost["tick_unit"] = json!("cntvct");
        assert!(validate(&cost, &product, "aarch64").is_ok());
        assert!(validate(&cost, &product, "ia32").is_err());
        assert!(validate(&cost, &product, "riscv64").is_err());
        cost["tick_unit"] = json!("tsc");
        assert!(validate(&cost, &product, "aarch64").is_err());
        cost["tick_unit"] = json!("cntvct");
        cost["plan_id"] = Value::Null;
        product["ordinary_plan_id"] = Value::Null;
        assert!(validate(&cost, &product, "aarch64").is_err());
    }
}

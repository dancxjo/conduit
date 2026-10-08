//! Require the actual Tour Play's independent output observation and bound cost.
use super::ConduitosError;
use serde_json::Value;
const FANOUT: &str = "CONDUIT_DOMAIN_MORSE_FANOUT original-input either-request-order independent-refusals changed-input-refused";
const HANDLES: &str = "CONDUIT_DOMAIN_MORSE_HANDLES text-indicator-cross-use-refused fault-revoked no-effect replay-refused";

pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "protected-morse-unproven",
            "require one canonical Tour observation and its exact completed domain cost",
        )
    };
    let parse = |prefix: &str| -> Result<Vec<Value>, ConduitosError> {
        transcript
            .lines()
            .filter_map(|line| line.strip_prefix(prefix))
            .map(|line| serde_json::from_str(line).map_err(|_| refusal()))
            .collect()
    };
    if ![HANDLES, FANOUT]
        .iter()
        .all(|expected| transcript.lines().any(|line| line == *expected))
    {
        return Err(refusal());
    }
    let signs = parse("CONDUIT_DOMAIN_MORSE ")?;
    let [sign] = signs.as_slice() else {
        return Err(refusal());
    };
    if sign["schema"] != "conduit.conduitos/protected-tour-morse@1"
        || sign["proof_class"] != "freestanding-emulator"
        || sign["canonical_pattern"] != true
        || sign["serial_effects"] != 2
        || sign["dma_isolation"] != false
        || sign["driver_isolation"] != false
    {
        return Err(refusal());
    }
    for field in [
        "plan_id",
        "play_id",
        "source_document_id",
        "checked_plot_id",
        "expanded_plot_id",
    ] {
        if !sign[field].as_str().is_some_and(|value| {
            value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(refusal());
        }
    }
    let costs = parse("CONDUIT_DOMAIN_COST ")?;
    let selected = costs
        .iter()
        .filter(|cost| {
            cost["fixture"] == false
                && cost["plan_id"] == sign["plan_id"]
                && cost["play_id"] == sign["play_id"]
        })
        .collect::<Vec<_>>();
    let [cost] = selected.as_slice() else {
        return Err(refusal());
    };
    let reserved = match sign["architecture"].as_str() {
        Some("x86_64" | "riscv64") => 217088,
        Some("aarch64" | "loongarch64") => 225280,
        Some("ia32") => 229376,
        _ => return Err(refusal()),
    };
    if cost["architecture"] != sign["architecture"]
        || cost["fixture"] != false
        || cost["entries"] != 5
        || cost["base_gate_transitions"] != 2
        || cost["gate_transitions"] != 5
        || cost["scheduler_returns"] != 5
        || cost["state"] != "Revoked(PlayCompleted)"
        || cost["reserved_bytes"] != reserved
        || cost["teardown_zeroed_bytes"] != reserved
        || cost["dma_isolation"] != false
        || cost["driver_isolation"] != false
    {
        return Err(refusal());
    }
    Ok(serde_json::json!({"observation": sign, "cost": cost}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn evidence() -> (Value, Value) {
        let sign = serde_json::json!({"schema":"conduit.conduitos/protected-tour-morse@1",
            "proof_class":"freestanding-emulator", "architecture":"x86_64", "canonical_pattern":true,"serial_effects":2,
            "dma_isolation":false,"driver_isolation":false,"plan_id":"11".repeat(32),
            "play_id":"22".repeat(32),"source_document_id":"33".repeat(32),
            "checked_plot_id":"44".repeat(32),"expanded_plot_id":"55".repeat(32)});
        let cost = serde_json::json!({"plan_id":sign["plan_id"],"play_id":sign["play_id"],
            "architecture":"x86_64","fixture":false,"entries":5,"base_gate_transitions":2,"gate_transitions":5,
            "scheduler_returns":5,"state":"Revoked(PlayCompleted)","reserved_bytes":217088,
            "teardown_zeroed_bytes":217088,"dma_isolation":false,"driver_isolation":false});
        (sign, cost)
    }
    fn transcript(sign: &Value, cost: &Value) -> String {
        format!("CONDUIT_DOMAIN_MORSE {sign}\nCONDUIT_DOMAIN_COST {cost}\n{HANDLES}\n{FANOUT}\n")
    }
    #[test]
    fn canonical_observation_requires_its_own_current_completed_cost() {
        let (sign, cost) = evidence();
        assert!(validate(&transcript(&sign, &cost)).is_ok());
        assert!(validate("").is_err());
        for missing in [HANDLES, FANOUT] {
            assert!(validate(&transcript(&sign, &cost).replace(missing, "")).is_err());
        }
        let mut fixture = cost.clone();
        fixture["fixture"] = Value::Bool(true);
        assert!(validate(
            &(transcript(&sign, &cost) + &format!("CONDUIT_DOMAIN_COST {fixture}\n"))
        )
        .is_ok());
        for field in [
            "plan_id",
            "play_id",
            "state",
            "fixture",
            "entries",
            "base_gate_transitions",
            "gate_transitions",
            "scheduler_returns",
            "teardown_zeroed_bytes",
            "dma_isolation",
        ] {
            let mut changed = cost.clone();
            changed[field] = Value::Null;
            assert!(validate(&transcript(&sign, &changed)).is_err(), "{field}");
        }
    }
    #[test]
    fn missing_duplicate_or_noncanonical_observations_do_not_prove_morse() {
        let (sign, cost) = evidence();
        let complete = transcript(&sign, &cost);
        assert!(validate(&(complete.clone() + &complete)).is_err());
        for field in [
            "source_document_id",
            "checked_plot_id",
            "expanded_plot_id",
            "canonical_pattern",
            "serial_effects",
            "proof_class",
            "driver_isolation",
        ] {
            let mut changed = sign.clone();
            changed[field] = Value::Null;
            assert!(validate(&transcript(&changed, &cost)).is_err(), "{field}");
        }
    }
}

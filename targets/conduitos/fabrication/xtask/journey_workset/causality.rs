//! Ordinary Boot and Body lifecycle identity checks, independent of foreground Form.
use super::{ConduitosError, Value};

pub(in super::super) fn validate(
    serial: &str,
    records: &[Value],
    opened: &Value,
    born: &Value,
    planned: &Value,
    playing: &Value,
    lulled: &Value,
) -> Result<(), ConduitosError> {
    let plan_id = planned["plan_id"].as_str().ok_or_else(|| {
        ConduitosError::refusal("product-journey-plan-absent", "planned identity absent")
    })?;
    let inspected_plan = serial.lines().any(|line| {
        line.contains("CONDUIT_FRONT_DOOR_SIGN")
            && line.contains("\"label\":\"PLAN ID\"")
            && line.contains(&format!("\"value\":\"{plan_id}\""))
    });
    if planned.get("active_play_id") != Some(&Value::Null)
        || planned
            .get("gear_ids")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        || planned
            .get("port_ids")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        || planned
            .get("cord_ids")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        || playing.get("active_play_id") == Some(&Value::Null)
        || playing.get("plan_id") == playing.get("active_play_id")
        || born.get("body_id") != lulled.get("body_id")
        || !inspected_plan
    {
        return Err(ConduitosError::refusal(
            "product-journey-causality-invalid",
            "exact Plan/Play/result/LULL causality did not match the product contract",
        ));
    }
    for identity in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
        let expected = opened.get(identity);
        if expected.is_none()
            || records
                .iter()
                .any(|record| record.get(identity) != expected)
        {
            return Err(ConduitosError::refusal(
                "product-journey-identity-drift",
                identity,
            ));
        }
    }
    Ok(())
}

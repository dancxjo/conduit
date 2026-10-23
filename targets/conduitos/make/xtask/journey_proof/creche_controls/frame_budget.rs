//! Costs of the current prepared Mask frame, not compositor or scanout history.
use super::*;
use conduit_face::{MAX_GRAPHICS_COMMANDS, MAX_GRAPHICS_TEXT_BYTES};
const BASIS: &str = "current prepared Mask frame used for focus; not compositor/scanout history";

fn cost(record: &Value) -> Result<(u64, u64, u64), ConduitosError> {
    let budget = &record["prepared_frame_budget"];
    let number = |key: &str| {
        budget[key]
            .as_u64()
            .ok_or_else(|| refusal("prepared-frame cost absent or nonnumeric"))
    };
    let commands = number("commands")?;
    let payload = number("payload_bytes")?;
    let maximum = number("maximum_payload_bytes")?;
    if commands == 0
        || commands > MAX_GRAPHICS_COMMANDS as u64
        || maximum > MAX_GRAPHICS_TEXT_BYTES as u64
        || payload < maximum
        || payload > commands * maximum
        || budget["command_limit"] != MAX_GRAPHICS_COMMANDS
        || budget["payload_byte_limit"] != MAX_GRAPHICS_TEXT_BYTES
        || budget["basis"] != BASIS
    {
        return Err(refusal(
            "prepared-frame cost exceeds exact bounds or has wrong scope",
        ));
    }
    Ok((commands, payload, maximum))
}
pub(super) fn summarize(serial: &str) -> Result<Value, ConduitosError> {
    let focuses = rows(serial, FOCUS)?;
    if focuses.is_empty() {
        return Err(refusal("prepared-frame observations absent"));
    }
    let (mut commands, mut payload, mut maximum) = (0u64, 0u64, 0u64);
    for focus in &focuses {
        let (c, p, m) = cost(focus)?;
        commands = commands.max(c);
        payload = payload.max(p);
        maximum = maximum.max(m);
    }
    Ok(json!({
        "schema":"conduit.native-prepared-frame-budget-proof/v1",
        "scope":BASIS, "focus_observations":focuses.len(),
        "command_high_water":commands,"command_limit":MAX_GRAPHICS_COMMANDS,
        "maximum_frame_payload_bytes":payload,"maximum_command_payload_bytes":maximum,
        "command_payload_byte_limit":MAX_GRAPHICS_TEXT_BYTES,
        "retained_compositor_surface_count":"not observed by this receipt"
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budget_guard_rejects_missing_overbound_or_misclassified_costs() {
        let good = json!({"prepared_frame_budget":{
            "commands":48,"command_limit":48,"payload_bytes":9216,
            "maximum_payload_bytes":192,"payload_byte_limit":192,"basis":BASIS}});
        assert_eq!(cost(&good).unwrap(), (48, 9216, 192));
        for (key, value) in [
            ("commands", json!(49)),
            ("commands", json!(0)),
            ("payload_bytes", json!(9217)),
            ("maximum_payload_bytes", json!(193)),
            ("payload_byte_limit", json!(256)),
            ("command_limit", json!(64)),
            ("maximum_payload_bytes", json!(null)),
            ("basis", json!("actual scanout")),
        ] {
            let mut bad = good.clone();
            bad["prepared_frame_budget"][key] = value;
            assert!(cost(&bad).is_err());
        }
        assert!(cost(&json!({})).is_err());
        let serial = format!("{FOCUS}{good}\n");
        assert_eq!(summarize(&serial).unwrap()["focus_observations"], 1);
        assert!(summarize("").is_err());
    }
}

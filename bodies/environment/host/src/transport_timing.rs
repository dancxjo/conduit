//! Opt-in scalar diagnostics; unrelated/private UART documents never reach logs.
use serde::Serialize;
use serde_json::Value;
const EVENT: &str = "body-checkpoint-timing";
const STAGES: usize = 18;
#[derive(Debug, Serialize)]
struct Timings {
    event: &'static str,
    success: bool,
    total_us: u64,
    visited: u32,
    last: u32,
    phase_us: [u32; STAGES],
}
fn checked(value: &Value) -> Option<Timings> {
    let object = value.as_object()?;
    let keys = ["event", "success", "total_us", "visited", "last", "phase_us"];
    if object.len() != keys.len()
        || !keys.iter().all(|key| object.contains_key(*key))
        || value["event"].as_str()? != EVENT
    {
        return None;
    }
    let visited = u32::try_from(value["visited"].as_u64()?).ok()?;
    let last = u32::try_from(value["last"].as_u64()?).ok()?;
    if visited >= 1 << STAGES || last >= STAGES as u32 || visited & (1 << last) == 0 {
        return None;
    }
    let phases = value["phase_us"].as_array()?;
    if phases.len() != STAGES {
        return None;
    }
    let mut phase_us = [0; STAGES];
    for (slot, item) in phase_us.iter_mut().zip(phases) {
        *slot = u32::try_from(item.as_u64()?).ok()?;
    }
    Some(Timings {
        event: EVENT,
        success: value["success"].as_bool()?,
        total_us: value["total_us"].as_u64()?,
        visited,
        last,
        phase_us,
    })
}
pub(super) fn observe(value: &Value) {
    if std::env::var("CONDUIT_ENVIRONMENT_CHECKPOINT_TIMING").as_deref() == Ok("1") {
        if let Some(timings) = checked(value) {
            if let Ok(line) = serde_json::to_string(&timings) {
                eprintln!("{line}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_extra_wrong_shape_and_out_of_range_fields_never_become_diagnostics() {
        let phases = vec![1_u32; 18];
        let valid = serde_json::json!({"event":EVENT,"success":true,"total_us":18,
            "visited":262143,"last":17,"phase_us":phases});
        assert!(checked(&valid).is_some());
        let mut extra = valid.clone();
        extra["invitation"] = serde_json::json!({"secret":"never log"});
        assert!(checked(&extra).is_none());
        for (key, bad) in [
            ("event", serde_json::json!("private-invitation")),
            ("success", serde_json::json!("true")),
            ("total_us", serde_json::json!(-1)),
            ("visited", serde_json::json!(262144)),
            ("visited", serde_json::json!(0)),
            ("last", serde_json::json!(18)),
            ("phase_us", serde_json::json!([0])),
            ("phase_us", serde_json::to_value([4294967296_u64;18]).unwrap()),
        ] {
            let mut value = valid.clone();
            value[key] = bad;
            assert!(checked(&value).is_none(), "{key}");
        }
        assert!(checked(&serde_json::json!({"invitation":{"secret":"private"}})).is_none());
    }
}

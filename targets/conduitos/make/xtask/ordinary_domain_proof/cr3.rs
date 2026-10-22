//! Exact pairing with already validated baseline rows; Root CR3 envelopes only.
use super::super::ConduitosError;
use serde_json::{json, Value};
const PREFIX: &str = "CONDUIT_SERVICE_CR3 ";
const SAMPLE: &str = "CONDUIT_SERVICE_SAMPLE ";
fn refused() -> ConduitosError {
    ConduitosError::refusal(
        "service-cr3-invalid",
        "requires twenty exactly paired raw Root CR3 envelopes and matched controls",
    )
}
pub(super) fn validate(text: &str, baseline: &Value) -> Result<Value, ConduitosError> {
    let lines: Vec<_> = text
        .split_inclusive('\n')
        .filter_map(|line| line.strip_suffix('\n'))
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| line.starts_with(PREFIX) || line.starts_with(SAMPLE))
        .collect();
    let samples = baseline["samples"]
        .as_array()
        .filter(|rows| rows.len() == 20)
        .ok_or_else(refused)?;
    if lines.len() != 40 {
        return Err(refused());
    }
    let mut rows = Vec::with_capacity(20);
    for (pair, expected) in lines.chunks_exact(2).zip(samples) {
        let sample: Value = serde_json::from_str(pair[0].strip_prefix(SAMPLE).ok_or_else(refused)?)
            .map_err(|_| refused())?;
        let row: Value = serde_json::from_str(pair[1].strip_prefix(PREFIX).ok_or_else(refused)?)
            .map_err(|_| refused())?;
        if &sample != expected {
            return Err(refused());
        }
        for key in ["mode", "case", "trial", "instance"] {
            if row[key] != expected[key] {
                return Err(refused());
            }
        }
        for (key, value) in [
            ("schema", json!("conduit.conduitos/service-cr3@1")),
            ("units", json!("raw-tsc")),
            ("calibrated", json!(false)),
            ("trap_attribution", json!(false)),
            ("counter_overhead_included", json!(true)),
            ("overhead_subtracted", json!(false)),
            ("guest_clock_authority_added", json!(false)),
        ] {
            if row[key] != value {
                return Err(refused());
            }
        }
        for key in [
            "entry_control",
            "entry_write",
            "return_control",
            "return_write",
        ] {
            if row[key].as_u64().is_none() {
                return Err(refused());
            }
        }
        rows.push(row);
    }
    Ok(
        json!({"samples":rows,"scope":"Root CR3 write envelopes with matched empty serialized counter/reconstruction controls; raw overhead included and not subtracted; instrumentation perturbs entry totals; not independent trap/register/IRQ cost, calibrated time, pure TLB-flush cost or product speedup"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<String>, Value) {
        let mut lines = Vec::new();
        let mut samples = Vec::new();
        let mut instance = 0;
        for index in 0..20 {
            let (case, mode, trial) = if index < 18 {
                (index / 6, (index % 6) / 3, index % 3)
            } else {
                (index - 15, 0, 0)
            };
            if mode == 0 || trial == 0 {
                instance += 1;
            }
            let sample = json!({"case":case,"mode":mode,"trial":trial,"instance":instance});
            let row = json!({"schema":"conduit.conduitos/service-cr3@1","units":"raw-tsc","calibrated":false,"trap_attribution":false,"counter_overhead_included":true,"overhead_subtracted":false,"guest_clock_authority_added":false,"case":case,"mode":mode,"trial":trial,"instance":instance,"entry_control":3,"entry_write":8,"return_control":4,"return_write":9});
            lines.push(format!("{SAMPLE}{sample}"));
            lines.push(format!("{PREFIX}{row}"));
            samples.push(sample);
        }
        (lines, json!({"samples":samples}))
    }
    fn text(lines: &[String]) -> String {
        lines.join("\n") + "\n"
    }
    #[test]
    fn exact_pairing_flags_and_unsigned_ticks_are_required() {
        let (lines, baseline) = fixture();
        let original = text(&lines);
        assert_eq!(
            validate(&original, &baseline).unwrap()["samples"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        for (key, value) in [
            ("schema", json!("future")),
            ("instance", json!(999)),
            ("case", json!(4)),
            ("calibrated", json!(true)),
            ("trap_attribution", json!(true)),
            ("counter_overhead_included", json!(false)),
            ("overhead_subtracted", json!(true)),
            ("guest_clock_authority_added", json!(true)),
            ("entry_write", json!(-1)),
            ("return_control", json!("4")),
            ("return_write", Value::Null),
        ] {
            let mut changed = lines.clone();
            let mut row: Value =
                serde_json::from_str(changed[1].strip_prefix(PREFIX).unwrap()).unwrap();
            assert_ne!(row[key], value);
            row[key] = value;
            changed[1] = format!("{PREFIX}{row}");
            assert!(validate(&text(&changed), &baseline).is_err(), "{key}");
        }
        let mut changed = baseline.clone();
        changed["samples"][0]["instance"] = json!(999);
        assert!(validate(&original, &changed).is_err());
    }
    #[test]
    fn missing_duplicate_reordered_and_complete_malformed_records_refuse() {
        let (lines, baseline) = fixture();
        let original = text(&lines);
        let mut changed = lines.clone();
        changed.remove(1);
        assert!(validate(&text(&changed), &baseline).is_err());
        let mut changed = lines.clone();
        changed.push(lines[1].clone());
        assert!(validate(&text(&changed), &baseline).is_err());
        let mut changed = lines.clone();
        changed.swap(1, 3);
        assert!(validate(&text(&changed), &baseline).is_err());
        assert!(validate(original.trim_end(), &baseline).is_err());
        assert!(validate(&format!("{original}{PREFIX}{{\n"), &baseline).is_err());
        assert!(validate(&format!("{original}{PREFIX}{{"), &baseline).is_ok());
        assert!(validate(&original.replace('\n', "\r\n"), &baseline).is_ok());
        assert!(validate(&format!("unrelated boot line\n{original}"), &baseline).is_ok());
    }
}

//! Exact pairing with already validated baseline rows; Root register/IRQ envelopes only.
use super::super::ConduitosError;
use serde_json::{json, Value};
const PREFIX: &str = "CONDUIT_SERVICE_ROOT_STATE ";
const SAMPLE: &str = "CONDUIT_SERVICE_SAMPLE ";
fn refused() -> ConduitosError {
    ConduitosError::refusal(
        "service-root-state-invalid",
        "requires twenty paired state envelopes and exact backend interrupt correspondence",
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
            ("schema", json!("conduit.conduitos/service-root-state@1")),
            ("units", json!("raw-tsc")),
            ("calibrated", json!(false)),
            ("pure_trap_attribution", json!(false)),
            ("registers", json!(6)),
            ("flags_cost_included", json!(false)),
            ("stack_switch_cost_included", json!(false)),
            ("irq_scope", json!("user-origin-root-handler")),
            ("counter_overhead_included", json!(true)),
            ("overhead_subtracted", json!(false)),
        ] {
            if row[key] != value {
                return Err(refused());
            }
        }
        for key in [
            "push_control",
            "pushes",
            "pop_control",
            "pops",
            "irq_count",
            "irq_total",
            "irq_control_total",
        ] {
            if row[key].as_u64().is_none() {
                return Err(refused());
            }
        }
        let count = row["irq_count"].as_u64().ok_or_else(refused)?;
        let total = row["irq_total"].as_u64().ok_or_else(refused)?;
        if row["irq_count"] != expected["interrupts"] {
            return Err(refused());
        }
        if count == 0 {
            if total != 0
                || row["irq_control_total"] != 0
                || !row.get("irq_min").is_some_and(Value::is_null)
                || !row.get("irq_max").is_some_and(Value::is_null)
            {
                return Err(refused());
            }
        } else {
            let min = row["irq_min"].as_u64().ok_or_else(refused)?;
            let max = row["irq_max"].as_u64().ok_or_else(refused)?;
            if min > max
                || u128::from(min) * u128::from(count) > u128::from(total)
                || u128::from(max) * u128::from(count) < u128::from(total)
            {
                return Err(refused());
            }
        }
        rows.push(row);
    }
    Ok(
        json!({"samples":rows,"scope":"Six callee-saved register pushes and six pops with separate empty controls; excludes flags and stack-switch cost. IRQ envelope covers only user-origin Root handler service after guest registers are saved; exact backend interrupt count and checked finite aggregates. Raw overhead included, no subtraction; not whole register preservation, total IRQ, pure trap, calibrated time or product gain."}),
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
                (index / 6, index % 6 / 3, index % 3)
            } else {
                (index - 15, 0, 0)
            };
            if mode == 0 || trial == 0 {
                instance += 1;
            }
            let count = if index == 18 { 2 } else { 0 };
            let sample = json!({"case":case,"mode":mode,"trial":trial,"instance":instance,"interrupts":count});
            let row = json!({"schema":"conduit.conduitos/service-root-state@1","units":"raw-tsc","calibrated":false,"registers":6,"flags_cost_included":false,"stack_switch_cost_included":false,"pure_trap_attribution":false,"counter_overhead_included":true,"overhead_subtracted":false,"irq_scope":"user-origin-root-handler","case":case,"mode":mode,"trial":trial,"instance":instance,"push_control":3,"pushes":8,"pop_control":4,"pops":9,"irq_count":count,"irq_total":if count==0{0}else{10},"irq_min":if count==0{Value::Null}else{json!(4)},"irq_max":if count==0{Value::Null}else{json!(6)},"irq_control_total":if count==0{0}else{3}});
            lines.push(format!("{SAMPLE}{sample}"));
            lines.push(format!("{PREFIX}{row}"));
            samples.push(sample);
        }
        (lines, json!({"samples":samples}))
    }
    fn text(lines: &[String]) -> String {
        lines.join("\n") + "\n"
    }
    fn change(lines: &[String], index: usize, key: &str, value: Value) -> Vec<String> {
        let mut lines = lines.to_vec();
        let mut row: Value =
            serde_json::from_str(lines[index].strip_prefix(PREFIX).unwrap()).unwrap();
        assert_ne!(row[key], value);
        row[key] = value;
        lines[index] = format!("{PREFIX}{row}");
        lines
    }
    #[test]
    fn exact_irq_counts_bounds_nullable_absence_and_scope_are_required() {
        let (lines, baseline) = fixture();
        assert_eq!(
            validate(&text(&lines), &baseline).unwrap()["samples"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        for (index, key, value) in [
            (1, "instance", json!(999)),
            (1, "registers", json!(7)),
            (1, "flags_cost_included", json!(true)),
            (1, "pure_trap_attribution", json!(true)),
            (1, "irq_count", json!(1)),
            (1, "irq_total", json!(1)),
            (1, "irq_control_total", json!(1)),
            (1, "irq_min", json!(0)),
            (1, "pushes", json!(-1)),
            (37, "irq_count", json!(3)),
            (37, "irq_total", json!(7)),
            (37, "irq_min", Value::Null),
            (37, "irq_max", json!(3)),
        ] {
            assert!(
                validate(&text(&change(&lines, index, key, value)), &baseline).is_err(),
                "{index}:{key}"
            );
        }
        let mut missing = lines.clone();
        let mut row: Value =
            serde_json::from_str(missing[1].strip_prefix(PREFIX).unwrap()).unwrap();
        row.as_object_mut().unwrap().remove("irq_min");
        missing[1] = format!("{PREFIX}{row}");
        assert!(validate(&text(&missing), &baseline).is_err());
    }
    #[test]
    fn pair_order_counts_complete_malformed_and_partial_tail_are_distinct() {
        let (lines, baseline) = fixture();
        let original = text(&lines);
        let mut changed = lines.clone();
        changed.swap(1, 3);
        assert!(validate(&text(&changed), &baseline).is_err());
        let mut changed = lines.clone();
        changed.remove(1);
        assert!(validate(&text(&changed), &baseline).is_err());
        assert!(validate(&format!("{original}{}\n", lines[1]), &baseline).is_err());
        assert!(validate(&format!("{original}{PREFIX}{{\n"), &baseline).is_err());
        assert!(validate(&format!("{original}{PREFIX}{{"), &baseline).is_ok());
        assert!(validate(original.trim_end(), &baseline).is_err());
        assert!(validate(&original.replace('\n', "\r\n"), &baseline).is_ok());
        assert!(validate(&format!("unrelated boot line\n{original}"), &baseline).is_ok());
    }
}

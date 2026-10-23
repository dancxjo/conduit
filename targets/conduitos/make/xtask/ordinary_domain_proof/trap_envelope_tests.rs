use super::*;
use serde_json::Value;

fn rows() -> Vec<Value> {
    let mut instance = 0;
    (0..20)
        .map(|index| {
            let (case, mode, trial) = if index < 18 {
                (index / 6, (index % 6) / 3, index % 3)
            } else {
                (index - 15, 0, 0)
            };
            if mode == 0 || trial == 0 {
                instance += 1;
            }
            let (value, origin) = match case {
                3 => (4, 2),
                4 => (0x10d, 1),
                _ => (0, 0),
            };
            serde_json::json!({"schema":"conduit.conduitos/service-trap-envelope@1", "units":"raw-tsc",
            "calibrated":false,"counter_overhead_included":true,"overhead_subtracted":false,
            "pure_hardware_trap":false,"full_irq_preservation":false,
            "row":index,"mode":mode,"case":case,"trial":trial,"instance":instance,
            "value":value,"origin":origin,"valid":true,"root_flags_preserved":true,"invalid":null,
            "entry_state_ticks":0,"terminal_return_ticks":if case==3 {Value::Null} else {serde_json::json!(20)},
            "root_restore_ticks":10,"entry_control_ticks":0,"return_control_ticks":0})
        })
        .collect()
}
fn text(rows: &[Value]) -> String {
    rows.iter().map(|row| format!("{PREFIX}{row}\n")).collect()
}
#[test]
fn accepts_exact_matrix_with_zero_measurement_and_absent_preempt_terminal() {
    let result = validate(&text(&rows())).unwrap();
    assert_eq!(result["rows"].as_array().unwrap().len(), 20);
    assert_eq!(result["intervals_additive"], false);
}
#[test]
fn refuses_missing_duplicate_reordered_and_foreign_instances() {
    let good = rows();
    assert_eq!(validate(&text(&good[..19])), Err(Refusal::Count));
    let mut bad = good.clone();
    bad.push(good[0].clone());
    assert_eq!(validate(&text(&bad)), Err(Refusal::Count));
    let mut bad = good.clone();
    bad.swap(0, 1);
    assert!(validate(&text(&bad)).is_err());
    for index in [0, 4, 6, 18, 19] {
        let mut bad = good.clone();
        bad[index]["instance"] = serde_json::json!(0);
        assert_eq!(validate(&text(&bad)), Err(Refusal::Identity));
    }
}
#[test]
fn every_nullable_field_is_required_and_only_preempt_terminal_can_be_null() {
    for field in [
        "invalid",
        "entry_state_ticks",
        "terminal_return_ticks",
        "root_restore_ticks",
        "entry_control_ticks",
        "return_control_ticks",
    ] {
        for index in [0, 18, 19] {
            let mut bad = rows();
            bad[index].as_object_mut().unwrap().remove(field);
            assert_eq!(
                validate(&text(&bad)),
                Err(Refusal::Malformed),
                "{field} row {index}"
            );
            if field != "invalid" && !(field == "terminal_return_ticks" && index == 18) {
                let mut bad = rows();
                bad[index][field] = Value::Null;
                assert_eq!(validate(&text(&bad)), Err(Refusal::Timing));
            }
        }
    }
    let mut bad = rows();
    bad[18]["terminal_return_ticks"] = serde_json::json!(0);
    assert_eq!(validate(&text(&bad)), Err(Refusal::Timing));
}
#[test]
fn refuses_invalid_clock_path_flags_and_corrupt_returns() {
    for reason in [
        "missing",
        "incomplete",
        "clock-regressed",
        "path-mismatch",
        "flags-mismatch",
    ] {
        let mut bad = rows();
        bad[0]["invalid"] = serde_json::json!(reason);
        assert_eq!(validate(&text(&bad)), Err(Refusal::Contract));
    }
    for field in ["valid", "root_flags_preserved"] {
        let mut bad = rows();
        bad[0][field] = serde_json::json!(false);
        assert_eq!(validate(&text(&bad)), Err(Refusal::Contract));
    }
    for index in [0, 18, 19] {
        for field in ["value", "origin"] {
            let mut bad = rows();
            bad[index][field] = serde_json::json!(999);
            assert_eq!(validate(&text(&bad)), Err(Refusal::Contract));
        }
    }
}
#[test]
fn refuses_unknown_duplicate_overflow_oversized_and_truncated_records() {
    let good = text(&rows());
    assert_eq!(validate(good.trim_end()), Err(Refusal::Truncated));
    for replacement in ["\"row\":0,\"row\":0", "\"unknown\":0,\"row\":0"] {
        assert_eq!(
            validate(&good.replacen("\"row\":0", replacement, 1)),
            Err(Refusal::Malformed)
        );
    }
    let overflow = good.replacen(
        "\"entry_state_ticks\":0",
        "\"entry_state_ticks\":18446744073709551616",
        1,
    );
    assert_eq!(validate(&overflow), Err(Refusal::Malformed));
    let huge = good.replacen(
        "\"units\":\"raw-tsc\"",
        &format!("\"units\":\"{}\"", "x".repeat(4096)),
        1,
    );
    assert_eq!(validate(&huge), Err(Refusal::Malformed));
}

#[test]
fn refuses_broadened_claims_and_impossible_nested_intervals() {
    for (field, value) in [
        ("calibrated", true),
        ("counter_overhead_included", false),
        ("overhead_subtracted", true),
        ("pure_hardware_trap", true),
        ("full_irq_preservation", true),
    ] {
        let mut bad = rows();
        bad[0][field] = serde_json::json!(value);
        assert_eq!(validate(&text(&bad)), Err(Refusal::Contract));
    }
    let mut bad = rows();
    bad[0]["root_restore_ticks"] = serde_json::json!(21);
    assert_eq!(validate(&text(&bad)), Err(Refusal::Timing));
    let mut bad = rows();
    bad[0]["invalid"] = serde_json::json!("unknown");
    assert_eq!(validate(&text(&bad)), Err(Refusal::Malformed));
}

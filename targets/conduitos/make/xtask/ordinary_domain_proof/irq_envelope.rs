//! Strict finite software IRQ samples, never physical interrupt latency.
use serde::Deserialize;
use serde_json::{json, Value};
const PREFIX: &str = "CONDUIT_SERVICE_SOFTWARE_IRQ ";
const CONTROL: &str = "CONDUIT_SERVICE_SOFTWARE_IRQ_CONTROL ";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    instance: u64,
    guest_sha256: String,
    trial: usize,
    ordinal: usize,
    start: u64,
    end: u64,
    budget: bool,
    preempted: bool,
    saved_gprs_and_rflags_preserved: bool,
    guest_canary_range: bool,
    overhead_included: bool,
    hardware_irq_latency: bool,
    cpu_entry_and_iret_excluded: bool,
}
pub(super) fn validate(serial: &str, guest: &str) -> Result<Value, &'static str> {
    let mut values = Vec::new();
    let mut trial = 0;
    let mut ordinal = 0;
    let mut instance = 0;
    let mut previous_instance = 0;
    let mut last_end = 0;
    let mut canary = false;
    let mut preempt = false;
    for line in serial.split_inclusive('\n') {
        if let Some(payload) = line.strip_prefix(PREFIX) {
            if !line.ends_with('\n') || line.len() > 2048 || trial >= 6 || ordinal == 64 || preempt
            {
                return Err("sample-bound");
            }
            let row: Record = serde_json::from_str(payload).map_err(|_| "sample-shape")?;
            if row.schema != "conduit.conduitos/service-software-irq@1"
                || row.instance == 0
                || row.guest_sha256 != guest
                || row.trial != trial
                || row.ordinal != ordinal
                || row.start < last_end
                || row.start == 0
                || row.end < row.start
                || !row.saved_gprs_and_rflags_preserved
                || !row.overhead_included
                || row.hardware_irq_latency
                || !row.cpu_entry_and_iret_excluded
                || row.preempted && !row.budget
            {
                return Err("sample-scope");
            }
            if ordinal == 0 {
                if row.instance <= previous_instance {
                    return Err("fresh-instance");
                }
                instance = row.instance;
            } else if row.instance != instance {
                return Err("instance-custody");
            }
            canary |= row.guest_canary_range && !row.preempted;
            preempt = row.preempted;
            last_end = row.end;
            ordinal += 1;
            values.push(serde_json::from_str::<Value>(payload).map_err(|_| "sample-value")?);
        } else if let Some(control) = line.strip_prefix(CONTROL) {
            if !line.ends_with('\n') || ordinal == 0 || trial >= 6 {
                return Err("control-order");
            }
            let expected = if trial < 3 {
                if !canary || preempt {
                    return Err("resume-not-proved");
                }
                "resume-gprs15-and-df-checked\n"
            } else {
                if !preempt {
                    return Err("preempt-not-proved");
                }
                "preempt-terminal-old-owner-refused\n"
            };
            if control != expected {
                return Err("control-scope");
            }
            previous_instance = instance;
            trial += 1;
            ordinal = 0;
            last_end = 0;
            canary = false;
            preempt = false;
        }
    }
    if trial != 6 || ordinal != 0 {
        return Err("incomplete-controls");
    }
    Ok(
        json!({"samples":values,"trials":6,"clock":"uncalibrated-raw-tsc",
        "scope":"software-irq-save-handler-restore-envelope",
        "excluded":"cpu-entry,vector-stub-word,initial-probe-saves,final-probe-restores,source-word-discard,iret",
        "overhead_included":true,"hardware_irq_latency":false,"full_xsave_preservation":false,
        "resume_gprs":15,"guest_flags_mask":"CF,PF,AF,ZF,SF,IF,DF,OF",
        "existing_envelopes_nonadditive":true}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn specimen() -> String {
        let mut lines = String::new();
        for trial in 0..6 {
            let preempt = trial >= 3;
            let row = json!({"schema":"conduit.conduitos/service-software-irq@1","instance":100+trial,
                "guest_sha256":"a".repeat(64),"trial":trial,"ordinal":0,"start":10,"end":20,
                "budget":true,"preempted":preempt,"saved_gprs_and_rflags_preserved":true,
                "guest_canary_range":!preempt,"overhead_included":true,"hardware_irq_latency":false,
                "cpu_entry_and_iret_excluded":true});
            lines += &format!(
                "{PREFIX}{row}\n{CONTROL}{}\n",
                if preempt {
                    "preempt-terminal-old-owner-refused"
                } else {
                    "resume-gprs15-and-df-checked"
                }
            );
        }
        lines
    }
    #[test]
    fn strict_software_irq_controls() {
        let good = specimen();
        let guest = "a".repeat(64);
        assert!(validate(&good, &guest).is_ok());
        for bad in [
            good.replacen("\"start\":10", "\"start\":30", 1),
            good.replacen(
                "\"saved_gprs_and_rflags_preserved\":true",
                "\"saved_gprs_and_rflags_preserved\":false",
                1,
            ),
            good.replacen(
                "\"guest_canary_range\":true",
                "\"guest_canary_range\":false",
                1,
            ),
            good.replacen(
                "\"hardware_irq_latency\":false",
                "\"hardware_irq_latency\":true",
                1,
            ),
            good.replacen("\"instance\":101", "\"instance\":100", 1),
            good.replacen("\"ordinal\":0", "\"ordinal\":1", 1),
            good.trim_end().to_owned(),
        ] {
            assert!(validate(&bad, &guest).is_err());
        }
        assert!(validate(&good, &"b".repeat(64)).is_err());
    }
}

//! Strict source/ELF-correlated collector; guest timestamps never confer authority.
use serde::Deserialize;
use serde_json::{json, Value};
const PREFIX: &str = "CONDUIT_SERVICE_PRIVILEGE_BOUNDARY ";
const CONTROL: &str = "CONDUIT_SERVICE_PRIVILEGE_BOUNDARY_CONTROL ";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    row: usize,
    instance: u64,
    guest_sha256: String,
    value: u64,
    origin: u64,
    valid: bool,
    root_cr4_preserved: bool,
    stamps: [u64; 4],
    root_to_user_ticks: u64,
    user_to_root_ticks: u64,
    clock: String,
    overhead_included: bool,
    pure_hardware: bool,
    guest_clock_permission: String,
    guest_timestamp_storage: String,
    budget_interrupts: u32,
    source_interrupts: u32,
    interrupts_included: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    schema: String,
    rows: usize,
    unarmed_clock_traps: bool,
    old_instance_terminal: bool,
    generic_install_ineligible: bool,
    generic_install_clock_traps: bool,
    fault_root_cr4_preserved: bool,
}
pub(super) fn validate(serial: &str) -> Result<Value, &'static str> {
    let mut records: Vec<Record> = Vec::with_capacity(18);
    let mut values = Vec::with_capacity(18);
    let mut control = false;
    for line in serial.split_inclusive('\n') {
        if let Some(payload) = line.strip_prefix(PREFIX) {
            if !line.ends_with('\n') || line.len() > 4096 || records.len() == 18 || control {
                return Err("record-bound");
            }
            let row: Record = serde_json::from_str(payload).map_err(|_| "record-shape")?;
            if row.row != records.len()
                || row.instance == 0
                || row.schema != "conduit.conduitos/service-privilege-boundary@1"
                || row.guest_sha256.len() != 64
                || row.guest_sha256.bytes().all(|byte| byte == b'0')
                || !row
                    .guest_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || row.value != 0
                || row.origin != 0
                || !row.valid
                || !row.root_cr4_preserved
                || row.stamps.contains(&0)
                || row.stamps.windows(2).any(|pair| pair[1] < pair[0])
                || row.stamps[1].checked_sub(row.stamps[0]) != Some(row.root_to_user_ticks)
                || row.stamps[3].checked_sub(row.stamps[2]) != Some(row.user_to_root_ticks)
                || row.clock != "uncalibrated-raw-tsc"
                || !row.overhead_included
                || row.pure_hardware
                || row.guest_clock_permission != "exact-image-one-entry"
                || row.guest_timestamp_storage != "reserved-r12-r13"
                || !row.interrupts_included
            {
                return Err("measurement-scope");
            }
            if let Some(previous) = records.last() {
                if row.guest_sha256 != previous.guest_sha256 {
                    return Err("image-binding");
                }
                let reused = row.row % 6 >= 4;
                if (reused && row.instance != previous.instance)
                    || (!reused && row.instance <= previous.instance)
                {
                    return Err("instance-binding");
                }
            }
            values.push(serde_json::from_str::<Value>(payload).map_err(|_| "record-value")?);
            records.push(row);
        } else if let Some(payload) = line.strip_prefix(CONTROL) {
            let result: Control = serde_json::from_str(payload).map_err(|_| "control-shape")?;
            if !line.ends_with('\n')
                || line.len() > 4096
                || control
                || records.len() != 18
                || result.schema != "conduit.conduitos/service-privilege-boundary-control@1"
                || result.rows != 18
                || !result.unarmed_clock_traps
                || !result.old_instance_terminal
                || !result.generic_install_ineligible
                || !result.generic_install_clock_traps
                || !result.fault_root_cr4_preserved
            {
                return Err("control");
            }
            control = true;
        } else if line.starts_with(PREFIX.trim_end()) || line.starts_with(CONTROL.trim_end()) {
            return Err("partial-prefix");
        }
    }
    if records.len() != 18 || !control {
        return Err("missing");
    }
    Ok(
        json!({"records":values,"unarmed_control":true,"clock":"raw-tsc","calibrated":false,
        "pure_hardware":false,"hardware_transition_included":true,"overhead_subtracted":false,
        "entry_envelope":"last Root timestamp through IRET plus guest entry branch/fences to first CPL3 timestamp",
        "return_envelope":"last CPL3 timestamp plus reconstruction/status/INT, hardware transition, CS guard and scratch pushes to first Root timestamp",
        "guest_clock_scope":"exact Root-compiled fixture bytes, owner-local one-shot arm, never generic install",
        "physical_latency_claim":false,
        "interrupt_observations":records.iter().map(|row|json!({"row":row.row,"budget":row.budget_interrupts,"source":row.source_interrupts})).collect::<Vec<_>>()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> String {
        let mut serial = String::new();
        for row in 0..18 {
            let instance = (row / 6) * 4 + if row % 6 < 3 { row % 6 + 1 } else { 4 };
            let record = json!({"schema":"conduit.conduitos/service-privilege-boundary@1","row":row,
                "instance":instance,"guest_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "value":0,"origin":0,"valid":true,"root_cr4_preserved":true,"stamps":[10,20,30,40],
                "root_to_user_ticks":10,"user_to_root_ticks":10,"clock":"uncalibrated-raw-tsc",
                "overhead_included":true,"pure_hardware":false,"guest_clock_permission":"exact-image-one-entry",
                "guest_timestamp_storage":"reserved-r12-r13","budget_interrupts":0,"source_interrupts":0,"interrupts_included":true});
            serial.push_str(&format!("{PREFIX}{record}\n"));
        }
        serial.push_str(&format!("{CONTROL}{}\n",json!({"schema":"conduit.conduitos/service-privilege-boundary-control@1",
            "rows":18,"unarmed_clock_traps":true,"old_instance_terminal":true,"generic_install_ineligible":true,"generic_install_clock_traps":true,"fault_root_cr4_preserved":true})));
        serial
    }
    #[test]
    fn complete_finite_rows_reject_missing_stale_instance_or_ambient_permission() {
        let good = fixture();
        assert!(validate(&good).is_ok());
        assert!(validate("").is_err());
        assert!(validate(good.trim_end()).is_err());
        assert!(validate(&format!("{good}{good}")).is_err());
        for (before, after) in [
            ("\"instance\":4", "\"instance\":3"),
            (
                "\"root_cr4_preserved\":true",
                "\"root_cr4_preserved\":false",
            ),
            (
                "\"generic_install_ineligible\":true",
                "\"generic_install_ineligible\":false",
            ),
            ("exact-image-one-entry", "ambient-allow"),
            ("[10,20,30,40]", "[10,9,30,40]"),
            ("\"pure_hardware\":false", "\"pure_hardware\":true"),
        ] {
            assert!(validate(&good.replace(before, after)).is_err(), "{before}");
        }
    }
}

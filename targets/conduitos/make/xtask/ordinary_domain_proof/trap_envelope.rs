//! Strict reader for inclusive software envelopes, never hardware trap latency.
extern crate std;
use self::std::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "CONDUIT_SERVICE_TRAP_ENVELOPE ";
const MAX_LINE: usize = 4096;

// Untagged buffering deliberately rejects Serde's missing-field sentinel.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum Present<T> {
    Null(()),
    Value(T),
}
impl<T> Present<T> {
    fn value(&self) -> Option<&T> {
        match self {
            Self::Null(()) => None,
            Self::Value(value) => Some(value),
        }
    }
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Invalid {
    Missing,
    Incomplete,
    ClockRegressed,
    PathMismatch,
    FlagsMismatch,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Row {
    schema: String,
    units: String,
    calibrated: bool,
    counter_overhead_included: bool,
    overhead_subtracted: bool,
    pure_hardware_trap: bool,
    full_irq_preservation: bool,
    row: u8,
    mode: u8,
    case: u8,
    trial: u8,
    instance: u64,
    value: u64,
    origin: u64,
    valid: bool,
    root_flags_preserved: bool,
    invalid: Present<Invalid>,
    entry_state_ticks: Present<u64>,
    terminal_return_ticks: Present<u64>,
    root_restore_ticks: Present<u64>,
    entry_control_ticks: Present<u64>,
    return_control_ticks: Present<u64>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Count,
    Truncated,
    Malformed,
    Contract,
    Identity,
    Timing,
}

pub(super) fn validate(transcript: &str) -> Result<serde_json::Value, Refusal> {
    let mut rows: Vec<Row> = Vec::with_capacity(20);
    for line in transcript.split_inclusive('\n') {
        let Some(payload) = line.strip_prefix(PREFIX) else {
            continue;
        };
        if !payload.ends_with('\n') {
            return Err(Refusal::Truncated);
        }
        if payload.len() > MAX_LINE {
            return Err(Refusal::Malformed);
        }
        if rows.len() == 20 {
            return Err(Refusal::Count);
        }
        rows.push(serde_json::from_str(payload).map_err(|_| Refusal::Malformed)?);
    }
    if rows.len() != 20 {
        return Err(Refusal::Count);
    }
    let mut latest_instance = 0;
    for (index, row) in rows.iter().enumerate() {
        let (case, mode, trial) = if index < 18 {
            (
                (index / 6) as u8,
                ((index % 6) / 3) as u8,
                (index % 3) as u8,
            )
        } else {
            ((index - 15) as u8, 0, 0)
        };
        if row.schema != "conduit.conduitos/service-trap-envelope@1"
            || row.units != "raw-tsc"
            || row.calibrated
            || !row.counter_overhead_included
            || row.overhead_subtracted
            || row.pure_hardware_trap
            || row.full_irq_preservation
            || !row.valid
            || !row.root_flags_preserved
            || row.invalid.value().is_some()
            || row.row as usize != index
            || (row.case, row.mode, row.trial) != (case, mode, trial)
            || (row.value, row.origin)
                != match case {
                    3 => (4, 2),
                    4 => (0x10d, 1),
                    _ => (0, 0),
                }
        {
            return Err(Refusal::Contract);
        }
        if mode == 1 && trial > 0 {
            if row.instance != rows[index - 1].instance {
                return Err(Refusal::Identity);
            }
        } else {
            if row.instance <= latest_instance {
                return Err(Refusal::Identity);
            }
            latest_instance = row.instance;
        }
        if row.entry_state_ticks.value().is_none()
            || row.root_restore_ticks.value().is_none()
            || row.entry_control_ticks.value().is_none()
            || row.return_control_ticks.value().is_none()
            || row.terminal_return_ticks.value().is_none() != (case == 3)
        {
            return Err(Refusal::Timing);
        }
        if let (Some(restore), Some(terminal)) = (
            row.root_restore_ticks.value(),
            row.terminal_return_ticks.value(),
        ) {
            // Both end at the same actual post-POPFQ timestamp. The Root
            // restoration begins inside the terminal software-return envelope.
            if restore > terminal {
                return Err(Refusal::Timing);
            }
        }
    }
    Ok(serde_json::json!({
        "schema": "conduit.conduitos/service-trap-envelope-observations@1",
        "units": "raw-tsc", "calibrated": false,
        "counter_overhead_included": true, "overhead_subtracted": false,
        "pure_hardware_trap": false, "full_irq_preservation": false,
        "intervals_overlap": true, "intervals_additive": false,
        "boundaries": {
            "entry_state_ticks": "parked-arguments-before-cr4-save through post-ts-setup",
            "terminal_return_ticks": "post-cpl3-origin-check through post-popfq-before-ret",
            "root_restore_ticks": "parked-return-pair-before-cr4-restore through post-popfq-before-ret",
            "controls": "adjacent raw counter pairs; reported without subtraction",
        },
        "exclusions": ["hardware-trap-delivery", "caller-work", "pure-iret", "full-irq-preservation", "physical-latency"],
        "rows": rows,
    }))
}

#[cfg(test)]
#[path = "trap_envelope_tests.rs"]
mod tests;

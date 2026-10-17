//! Require real guest ring evidence, independently of descriptor/class acceptance.
use super::ConduitosError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RingProofSign {
    schema: String,
    proof_class: String,
    status: String,
    root_port: u8,
    slot: u8,
    transfers: u16,
    short_transfers: u16,
    cycle_transitions: u16,
    initial_enqueue: usize,
    initial_cycle: u32,
    final_enqueue: usize,
    final_cycle: u32,
    ring_trbs: u16,
    maximum_in_flight: u8,
    dma_bytes: u16,
    fixture_protocol: bool,
}

impl RingProofSign {
    pub(super) fn final_position(&self) -> (usize, u32) {
        (self.final_enqueue, self.final_cycle)
    }
}

pub(super) fn extract(
    serial: &str,
    root_port: u8,
    slot: u8,
) -> Result<RingProofSign, ConduitosError> {
    let mut signs = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_USB_RING_SIGN "));
    let one = signs.next().ok_or_else(|| {
        ConduitosError::refusal(
            "usb-ring-proof-missing",
            "guest did not complete reusable control-ring proof",
        )
    })?;
    if signs.next().is_some() {
        return Err(ConduitosError::refusal(
            "usb-ring-proof-duplicated",
            "expected one primary-device ring proof",
        ));
    }
    let sign: RingProofSign = serde_json::from_str(one)
        .map_err(|error| ConduitosError::refusal("usb-ring-proof-malformed", error.to_string()))?;
    let mut cursor = sign.initial_enqueue;
    let mut cycle = sign.initial_cycle;
    let mut transitions = 0;
    for _ in 0..64 {
        if cursor > 28 {
            cursor = 0;
            cycle ^= 1;
            transitions += 1;
        }
        cursor += 3;
    }
    if sign.schema != "conduit.conduitos.usb-control-ring/v1"
        || sign.proof_class != "freestanding-emulator"
        || sign.status != "completed"
        || sign.root_port != root_port
        || sign.slot != slot
        || sign.transfers != 64
        || sign.short_transfers != 64
        || sign.initial_enqueue != 14
        || sign.initial_cycle != 1
        || sign.cycle_transitions != transitions
        || transitions < 4
        || sign.final_enqueue != cursor
        || sign.final_cycle != cycle
        || sign.ring_trbs != 32
        || sign.maximum_in_flight != 1
        || sign.dma_bytes != 8192
        || !sign.fixture_protocol
    {
        return Err(ConduitosError::refusal(
            "usb-ring-proof-invalid",
            format!("ring proof differs from admitted fixed geometry: {sign:?}"),
        ));
    }
    Ok(sign)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::json!({"schema":"conduit.conduitos.usb-control-ring/v1","proof_class":"freestanding-emulator","status":"completed","root_port":1,"slot":1,"transfers":64,"short_transfers":64,"cycle_transitions":6,"initial_enqueue":14,"initial_cycle":1,"final_enqueue":27,"final_cycle":1,"ring_trbs":32,"maximum_in_flight":1,"dma_bytes":8192,"fixture_protocol":true})
    }
    fn serial(value: &serde_json::Value) -> String {
        format!("CONDUIT_USB_RING_SIGN {value}\n")
    }
    #[test]
    fn proof_requires_complete_cycles_and_the_exact_device_geometry() {
        assert!(extract(&serial(&fixture()), 1, 1).is_ok());
        assert!(extract(&serial(&fixture()), 2, 1).is_err());
        assert!(extract(&serial(&fixture()), 1, 2).is_err());
        for (field, value) in [
            ("proof_class", serde_json::json!("simulation")),
            ("transfers", serde_json::json!(1)),
            ("short_transfers", serde_json::json!(0)),
            ("cycle_transitions", serde_json::json!(0)),
            ("initial_enqueue", serde_json::json!(13)),
            ("final_enqueue", serde_json::json!(28)),
            ("final_cycle", serde_json::json!(0)),
            ("maximum_in_flight", serde_json::json!(2)),
            ("dma_bytes", serde_json::json!(16384)),
            ("fixture_protocol", serde_json::json!(false)),
        ] {
            let mut record = fixture();
            record[field] = value;
            assert!(extract(&serial(&record), 1, 1).is_err(), "{field}");
        }
    }
    #[test]
    fn missing_duplicate_or_malformed_evidence_does_not_issue_a_receipt() {
        assert!(extract("", 1, 1).is_err());
        assert!(extract("CONDUIT_USB_RING_SIGN {}\n", 1, 1).is_err());
        let sign = serial(&fixture());
        assert!(extract(&(sign.clone() + &sign), 1, 1).is_err());
    }
}

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

pub(super) const CONTRACT_ID: &str = "sparkfun-promicro-5v16-create1-minidin-uart@1";
const RECEIPT_SCHEMA: &str = "conduit.avr-promicro/create1-attachment-qualification@1";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttachmentQualification {
    schema: String,
    contract_id: String,
    source_sha: String,
    board: String,
    create: String,
    interface_id: String,
    interface_kind: InterfaceKind,
    measurement_instrument: String,
    promicro_vcc_mv: u16,
    create_txd_idle_mv: u16,
    common_ground_resistance_milliohms: u16,
    promicro_txo_to_create_rxd_minidin_pin: u8,
    create_txd_minidin_pin_to_promicro_rxi: u8,
    create_ground_minidin_pin: u8,
    create_vpwr_connected: bool,
    create_brc_connected: bool,
    boot_and_terminal_high_impedance: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum InterfaceKind {
    Direct5vTtl,
    Buffered5vTtl,
}

pub(super) fn load_and_validate(
    path: &Path,
    source_sha: &str,
) -> Result<AttachmentQualification, Box<dyn std::error::Error>> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "AVR Create HIL requires readable attachment qualification {}: {error}",
            path.display()
        )
    })?;
    let receipt: AttachmentQualification = serde_json::from_slice(&bytes)
        .map_err(|error| format!("malformed AVR attachment qualification: {error}"))?;
    receipt.validate(source_sha)?;
    Ok(receipt)
}

impl AttachmentQualification {
    fn validate(&self, source_sha: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.schema != RECEIPT_SCHEMA || self.contract_id != CONTRACT_ID {
            return Err("AVR attachment qualification names the wrong exact contract".into());
        }
        if self.source_sha != source_sha {
            return Err(format!(
                "AVR attachment qualification is stale: expected source {source_sha}, found {}",
                self.source_sha
            )
            .into());
        }
        if self.board != "sparkfun-promicro-atmega32u4-5v-16mhz"
            || self.create != "irobot-create-1-minidin"
        {
            return Err("AVR attachment qualification names the wrong endpoints".into());
        }
        if self.interface_id.trim().is_empty()
            || self.measurement_instrument.trim().is_empty()
            || self.interface_id.eq_ignore_ascii_case("none")
            || self.measurement_instrument.eq_ignore_ascii_case("none")
        {
            return Err(
                "AVR attachment qualification requires exact interface and instrument identities"
                    .into(),
            );
        }
        if !(4_750..=5_250).contains(&self.promicro_vcc_mv)
            || !(3_500..=5_250).contains(&self.create_txd_idle_mv)
        {
            return Err("AVR attachment qualification measured incompatible 5 V TTL levels".into());
        }
        if self.common_ground_resistance_milliohms > 1_000 {
            return Err("AVR attachment qualification did not establish a common ground".into());
        }
        if self.promicro_txo_to_create_rxd_minidin_pin != 3
            || self.create_txd_minidin_pin_to_promicro_rxi != 4
            || !matches!(self.create_ground_minidin_pin, 6 | 7)
        {
            return Err(
                "AVR attachment qualification does not match the crossed UART pin contract".into(),
            );
        }
        if self.create_vpwr_connected
            || self.create_brc_connected
            || !self.boot_and_terminal_high_impedance
        {
            return Err(
                "AVR attachment qualification violates the fail-closed power/output contract"
                    .into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact() -> AttachmentQualification {
        AttachmentQualification {
            schema: RECEIPT_SCHEMA.into(),
            contract_id: CONTRACT_ID.into(),
            source_sha: "a".repeat(40),
            board: "sparkfun-promicro-atmega32u4-5v-16mhz".into(),
            create: "irobot-create-1-minidin".into(),
            interface_id: "pete-uart-harness/rev-a/serial-001".into(),
            interface_kind: InterfaceKind::Direct5vTtl,
            measurement_instrument: "meter/serial-001/cal-2026-08-01".into(),
            promicro_vcc_mv: 5_010,
            create_txd_idle_mv: 4_980,
            common_ground_resistance_milliohms: 120,
            promicro_txo_to_create_rxd_minidin_pin: 3,
            create_txd_minidin_pin_to_promicro_rxi: 4,
            create_ground_minidin_pin: 6,
            create_vpwr_connected: false,
            create_brc_connected: false,
            boot_and_terminal_high_impedance: true,
        }
    }

    #[test]
    fn exact_direct_or_buffered_5v_contract_is_accepted() {
        let mut receipt = exact();
        receipt.validate(&"a".repeat(40)).unwrap();
        receipt.interface_kind = InterfaceKind::Buffered5vTtl;
        receipt.validate(&"a".repeat(40)).unwrap();
    }

    #[test]
    fn stale_incomplete_or_electrically_unsafe_contract_is_refused() {
        let mut receipt = exact();
        assert!(receipt.validate(&"b".repeat(40)).is_err());
        receipt = exact();
        receipt.interface_id.clear();
        assert!(receipt.validate(&"a".repeat(40)).is_err());
        receipt = exact();
        receipt.create_txd_idle_mv = 5_600;
        assert!(receipt.validate(&"a".repeat(40)).is_err());
        receipt = exact();
        receipt.common_ground_resistance_milliohms = 1_001;
        assert!(receipt.validate(&"a".repeat(40)).is_err());
        receipt = exact();
        receipt.create_vpwr_connected = true;
        assert!(receipt.validate(&"a".repeat(40)).is_err());
        receipt = exact();
        receipt.promicro_txo_to_create_rxd_minidin_pin = 4;
        assert!(receipt.validate(&"a".repeat(40)).is_err());
    }
}

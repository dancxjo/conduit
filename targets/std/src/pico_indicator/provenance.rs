//! Optional inspection evidence over the ordinary indicator capability.
use super::PicoIndicator;
use conduit_core::{
    DeviceAssociation, DeviceIdentityEvidence, DeviceIdentityFact, DeviceIdentityStrength,
    DeviceResourceProvenance, DeviceTruthDisposition, PROTOCOL_VERSION,
};
use std::fmt::Write;

impl PicoIndicator {
    /// Snapshot outside Play. This allocates inspection data, not semantic Info,
    /// and never creates an additional capability or grants device authority.
    /// Reacquisition produces a different pool/device identity even on the same
    /// physical board. A failed provider can expose only historical provenance.
    pub fn device_association(&self) -> DeviceAssociation {
        let provider = "conduit.std/pico-indicator-cdc@1";
        DeviceAssociation {
            protocol_version: PROTOCOL_VERSION,
            device_id: format!("pico/indicator-device:{}", self.binding.pool_id.as_str()).into(),
            host_id: self.binding.host_id.clone(),
            boot_id: self.binding.boot_id.clone(),
            offer_generation: self.binding.offer_generation,
            disposition: if self.failure.is_some() {
                DeviceTruthDisposition::HistoricalLost {
                    terminal_sign_id: None,
                }
            } else {
                DeviceTruthDisposition::Current
            },
            capability_ids: vec![conduit_std_offers::indicator_resource::IMPLEMENTATION.into()],
            resources: vec![DeviceResourceProvenance {
                handle_id: self.binding.pool_id.as_str().into(),
                class_id: conduit_std_offers::indicator_resource::RESOURCE_CLASS.into(),
                base_implementation_id: provider.into(),
                base_instance_id: self.binding.pool_id.as_str().into(),
            }],
            identity_evidence: DeviceIdentityEvidence {
                strength: DeviceIdentityStrength::BootLocalResource,
                provider: provider.into(),
                facts: vec![
                    DeviceIdentityFact {
                        name: "device-boot".into(),
                        value: hex(self.device_boot()),
                    },
                    DeviceIdentityFact {
                        name: "firmware-digest".into(),
                        value: hex(self.firmware_digest()),
                    },
                    DeviceIdentityFact {
                        name: "protocol".into(),
                        value: "pico-indicator/CIR1".into(),
                    },
                ],
            },
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(text, "{byte:02x}").expect("String formatting");
    }
    text
}

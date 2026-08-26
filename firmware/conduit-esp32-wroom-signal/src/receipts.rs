//! Allocation-free boot-scoped identity receipts for physical acceptance.

use esp_hal::{
    efuse::{self, InterfaceMacAddress},
    rng::Trng,
};
use trouble_host::prelude::Address;

use alloc::{format, string::String};

pub struct BootIdentity {
    host_mac: [u8; 6],
    nonce: [u8; 16],
}

impl BootIdentity {
    pub fn fresh(rng: &Trng) -> Self {
        let host = efuse::interface_mac_address(InterfaceMacAddress::Station);
        let mut host_mac = [0_u8; 6];
        host_mac.copy_from_slice(host.as_bytes());
        let mut nonce = [0_u8; 16];
        rng.read(&mut nonce);
        Self { host_mac, nonce }
    }

    pub fn print_boot(&self) {
        esp_println::println!(
            "CONDUIT_ESP32_BOOT schema=conduit.host/esp32-boot@1 host=esp32/{:02x}{:02x}{:02x}{:02x}{:02x}{:02x} boot={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x} plan={} fabrication={}",
            self.host_mac[0],
            self.host_mac[1],
            self.host_mac[2],
            self.host_mac[3],
            self.host_mac[4],
            self.host_mac[5],
            self.nonce[0],
            self.nonce[1],
            self.nonce[2],
            self.nonce[3],
            self.nonce[4],
            self.nonce[5],
            self.nonce[6],
            self.nonce[7],
            self.nonce[8],
            self.nonce[9],
            self.nonce[10],
            self.nonce[11],
            self.nonce[12],
            self.nonce[13],
            self.nonce[14],
            self.nonce[15],
            crate::generated::PLAN_ID,
            crate::generated::GENERATED_FABRICATION_DESCRIPTOR_BINDING,
        );
    }

    pub fn boot_id(&self) -> String {
        format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.nonce[0],
            self.nonce[1],
            self.nonce[2],
            self.nonce[3],
            self.nonce[4],
            self.nonce[5],
            self.nonce[6],
            self.nonce[7],
            self.nonce[8],
            self.nonce[9],
            self.nonce[10],
            self.nonce[11],
            self.nonce[12],
            self.nonce[13],
            self.nonce[14],
            self.nonce[15]
        )
    }

    pub fn print_host_offer(&self, address: Address) {
        esp_println::println!(
            "CONDUIT_ESP32_HOST schema=conduit.host/esp32-advertisement@1 host=esp32/{:02x}{:02x}{:02x}{:02x}{:02x}{:02x} boot={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x} generation=1 base=bluetooth-le-gatt base-instance=boot/ble-controller/0 address={} sessions=1 in-flight-items=1 payload-bytes={} buffered-bytes={} frame-bytes=2048 reconnect-attempts=0",
            self.host_mac[0],
            self.host_mac[1],
            self.host_mac[2],
            self.host_mac[3],
            self.host_mac[4],
            self.host_mac[5],
            self.nonce[0],
            self.nonce[1],
            self.nonce[2],
            self.nonce[3],
            self.nonce[4],
            self.nonce[5],
            self.nonce[6],
            self.nonce[7],
            self.nonce[8],
            self.nonce[9],
            self.nonce[10],
            self.nonce[11],
            self.nonce[12],
            self.nonce[13],
            self.nonce[14],
            self.nonce[15],
            address,
            crate::generated::CORD_VALUE_BYTES / u32::from(crate::generated::CORD_VALUE_SLOTS),
            crate::generated::CORD_VALUE_BYTES,
        );
    }
}

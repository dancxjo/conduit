//! Allocation-independent boot identity derivation.

use alloc::string::String;
use core::fmt::Write;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootIdentities {
    pub host: [u8; 32],
    pub boot: [u8; 32],
}

pub fn derive(entropy: [u64; 4], timestamp: u64, image_start: u64) -> BootIdentities {
    let host = digest(b"conduit-host-id/v1", &entropy, timestamp, image_start);
    let boot = digest(b"conduit-boot-id/v1", &entropy, timestamp, image_start);
    BootIdentities { host, boot }
}

pub fn derive_base(boot_id: &[u8; 32], kind: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((b"conduit-base-id/v1".len() as u32).to_le_bytes());
    hash.update(b"conduit-base-id/v1");
    hash.update(boot_id);
    hash.update((kind.len() as u32).to_le_bytes());
    hash.update(kind.as_bytes());
    hash.finalize().into()
}

pub fn derive_usb_device(
    boot_id: &[u8; 32],
    base_id: &[u8; 32],
    root_port: u8,
    slot: u8,
    attachment_epoch: u32,
) -> [u8; 32] {
    let mut facts = [0; 6];
    facts[0] = root_port;
    facts[1] = slot;
    facts[2..].copy_from_slice(&attachment_epoch.to_le_bytes());
    subject_digest(b"conduit-usb-device-instance/v1", boot_id, base_id, &facts)
}

pub fn derive_usb_interface(device_id: &[u8; 32], number: u8, alternate: u8) -> [u8; 32] {
    subject_digest(
        b"conduit-usb-interface/v1",
        device_id,
        &[0; 32],
        &[number, alternate],
    )
}

pub fn derive_usb_endpoint(interface_id: &[u8; 32], address: u8) -> [u8; 32] {
    subject_digest(
        b"conduit-usb-endpoint/v1",
        interface_id,
        &[0; 32],
        &[address],
    )
}

pub fn hex(bytes: &[u8; 32]) -> String {
    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

fn digest(domain: &[u8], entropy: &[u64; 4], timestamp: u64, image_start: u64) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((domain.len() as u32).to_le_bytes());
    hash.update(domain);
    for word in entropy {
        hash.update(word.to_le_bytes());
    }
    hash.update(timestamp.to_le_bytes());
    hash.update(image_start.to_le_bytes());
    hash.finalize().into()
}

fn subject_digest(domain: &[u8], first: &[u8; 32], second: &[u8; 32], facts: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update((domain.len() as u32).to_le_bytes());
    hash.update(domain);
    hash.update(first);
    hash.update(second);
    hash.update((facts.len() as u32).to_le_bytes());
    hash.update(facts);
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_separates_host_and_boot_identity() {
        let ids = derive([1, 2, 3, 4], 5, 6);
        assert_ne!(ids.host, ids.boot);
        assert_eq!(ids, derive([1, 2, 3, 4], 5, 6));
        assert_ne!(ids.boot, derive([1, 2, 3, 7], 5, 6).boot);
        assert_ne!(
            derive_base(&ids.boot, "timer"),
            derive_base(&ids.boot, "serial")
        );
    }

    #[test]
    fn usb_subject_identities_remain_exact_and_boot_local() {
        let boot = [1; 32];
        let base = derive_base(&boot, "xhci");
        let device = derive_usb_device(&boot, &base, 1, 1, 1);
        let interface = derive_usb_interface(&device, 0, 0);
        let endpoint = derive_usb_endpoint(&interface, 0x81);
        assert_ne!(device, base);
        assert_ne!(interface, device);
        assert_ne!(endpoint, interface);
        assert_ne!(device, derive_usb_device(&[2; 32], &base, 1, 1, 1));
        assert_ne!(device, derive_usb_device(&boot, &base, 1, 1, 2));
    }
}

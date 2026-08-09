//! Allocation-independent boot identity derivation.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_separates_host_and_boot_identity() {
        let ids = derive([1, 2, 3, 4], 5, 6);
        assert_ne!(ids.host, ids.boot);
        assert_eq!(ids, derive([1, 2, 3, 4], 5, 6));
        assert_ne!(ids.boot, derive([1, 2, 3, 7], 5, 6).boot);
    }
}

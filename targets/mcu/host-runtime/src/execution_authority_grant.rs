//! One bounded global identity for an original native grant description.
//! The existing fingerprint binds Body, Host, Boot, provider, IMAGE and operation.
use sha2::{Digest, Sha256};
pub(super) fn bind(label: &str, fingerprint: &[u8; 32]) -> [u8; 81] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.mcu/native-grant@1\0");
    hash.update(fingerprint);
    hash.update(label.as_bytes());
    let digest = hash.finalize();
    let mut identity = [0; 81];
    identity[..17].copy_from_slice(b"mcu/native-grant/");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, byte) in digest.iter().copied().enumerate() {
        identity[17 + i * 2] = HEX[usize::from(byte >> 4)];
        identity[18 + i * 2] = HEX[usize::from(byte & 15)];
    }
    identity
}

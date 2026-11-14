//! Bound and clear the raw package buffer, which includes invitation secrets.
use std::{io::Read, path::Path};
pub(super) struct SecretBytes(pub(super) Vec<u8>);
impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
pub(super) fn read(path: &Path, maximum: usize) -> std::io::Result<SecretBytes> {
    let mut bytes = SecretBytes(Vec::new());
    std::fs::File::open(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes.0)?;
    Ok(bytes)
}

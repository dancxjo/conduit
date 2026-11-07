//! Native facts for the trusted ESP32 Host composition; no authority is issued.
use esp_hal::{
    efuse,
    rng::{Trng, TrngError},
    time::Instant,
};

/// Factory eFuse identifier; unaffected by software interface-MAC overrides.
/// This identifies physical machinery, not a Body membership or authorization.
pub fn physical_uid() -> [u8; 6] {
    let mut uid = [0; 6];
    uid.copy_from_slice(efuse::base_mac_address().as_bytes());
    uid
}

/// Refuses when no actual TRNG source is retained by the composition. No
/// pseudo-random or wall-clock fallback can silently produce a Boot nonce.
pub fn fresh_boot_nonce() -> Result<[u8; 16], TrngError> {
    let rng = Trng::try_new()?;
    let mut nonce = [0; 16];
    rng.read(&mut nonce);
    Ok(nonce)
}

/// Local monotonic coordinate since this Boot, never a global wall-time claim.
pub fn monotonic_millis() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

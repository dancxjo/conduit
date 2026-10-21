//! Same ChaCha20 stream and volatile erasure used by the enclosing native carrier.
use rand_chacha::ChaCha20Rng;
use rand_core::{CryptoRng, RngCore, SeedableRng};
pub(crate) struct ErasedChaCha(ChaCha20Rng);
impl ErasedChaCha {
    pub fn new(seed: [u8; 32]) -> Self {
        Self(ChaCha20Rng::from_seed(seed))
    }
}
impl RngCore for ErasedChaCha {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
    fn fill_bytes(&mut self, out: &mut [u8]) {
        self.0.fill_bytes(out)
    }
    fn try_fill_bytes(&mut self, out: &mut [u8]) -> Result<(), rand_core::Error> {
        self.0.try_fill_bytes(out)
    }
}
impl CryptoRng for ErasedChaCha {}
impl Drop for ErasedChaCha {
    fn drop(&mut self) {
        let bytes = core::ptr::from_mut(&mut self.0).cast::<u8>();
        for offset in 0..core::mem::size_of::<ChaCha20Rng>() {
            // SAFETY: exclusive Drop owns the complete RNG representation and
            // never reads or uses that representation after volatile erasure.
            unsafe { core::ptr::write_volatile(bytes.add(offset), 0) };
        }
    }
}

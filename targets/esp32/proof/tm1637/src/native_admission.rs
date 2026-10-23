//! Existing Body ambient admission proof, without membership or effect authority.
use conduit_body::{ambient_admission_transcript, validate_pico_challenge, PicoAdmissionChallenge};
use conduit_core::OfferGeneration;
use ed25519_dalek::{Signer, SigningKey};

// This UART carrier admits a larger offer than the Pico USB carrier; the
// borrowed challenge/proof documents and Body verifier remain unchanged.
pub const MAXIMUM_ADVERTISEMENT_BYTES: usize = 8192;
pub struct NativeAdmission {
    key: SigningKey,
}
impl NativeAdmission {
    /// Caller supplies independently initialized hardware entropy. No fallback.
    pub fn new(mut seed: [u8; 32]) -> Result<Self, ()> {
        if seed == [0; 32] {
            return Err(());
        }
        let key = SigningKey::from_bytes(&seed);
        seed.fill(0);
        Ok(Self { key })
    }
    pub fn verifying_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
    /// Root supplies this only after exact reservation/lifecycle admission.
    pub fn sign_owner_receipt(&self, receipt: &crate::native_owner_receipt::Receipt) -> [u8; 64] {
        self.key.sign(&receipt.transcript()).to_bytes()
    }
    /// Expiry is checked by the Body authority in its own clock domain. This
    /// responder validates the exact current scope and signs no install/start.
    pub fn sign(
        &self,
        challenge: &PicoAdmissionChallenge<'_>,
        host: &str,
        boot: &str,
        generation: u64,
    ) -> Result<[u8; 64], ()> {
        if !validate_pico_challenge(challenge, host, boot, generation) {
            return Err(());
        }
        Ok(self
            .key
            .sign(&ambient_admission_transcript(
                "ambient-admission-v1",
                challenge.body_id,
                challenge.admission_id,
                host,
                boot,
                OfferGeneration(generation),
                &challenge.nonce,
                challenge.expires_at_millis,
            ))
            .to_bytes())
    }
}

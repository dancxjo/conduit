//! Common decoder for target-owned immutable membership construction carriers.
use conduit_body::MemberBootstrap;
use core::ops::Deref;
const MAGIC: &[u8; 16] = b"CONDUIT_SEED@2\0\0";
const HEADER: usize = 20;

/// Private material; deliberately neither Debug nor Clone. Parsing checks its
/// construction binding, not its current-Boot admission or execution authority.
pub struct Seed<'a>(MemberBootstrap<'a>);
impl<'a> Deref for Seed<'a> {
    type Target = MemberBootstrap<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> Seed<'a> {
    // Only original Root restores an already authenticated durable acceptance.
    // This does not create current-Boot online membership.
    #[cfg(feature = "execution")]
    pub(crate) fn replace_retained_credential(
        &mut self,
        credential: conduit_body::MembershipCredential,
    ) {
        let mut secret = *self.0.admission.secret();
        self.0.admission = conduit_body::MemberBootstrapMaterial::Retained { credential, secret };
        for byte in &mut secret {
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
    }
    /// The platform owns carrier address, extent, immutable lifetime and access.
    /// Native identities must come independently from its checked IMAGE.
    #[inline(never)]
    pub fn parse(
        bytes: &'a [u8],
        actual_host: &str,
        image: &str,
        configuration: &str,
    ) -> Option<Self> {
        if bytes.get(..16)? != MAGIC {
            return None;
        }
        let length = u32::from_le_bytes(bytes.get(16..HEADER)?.try_into().ok()?) as usize;
        if length == 0 {
            return None;
        }
        let payload = bytes.get(HEADER..HEADER.checked_add(length)?)?;
        let (seed, used) = serde_json_core::from_slice::<MemberBootstrap<'a>>(payload).ok()?;
        if used != length || !seed.matches_native(actual_host, image, configuration) {
            return None;
        }
        Some(Self(seed))
    }
}

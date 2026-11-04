//! Immutable target-owned carrier for shared membership construction material.
use conduit_body::MemberBootstrap;
use core::ops::Deref;
const MAGIC: &[u8; 16] = b"CONDUIT_SEED@2\0\0";
const HEADER: usize = 20;
pub struct Seed<'a>(MemberBootstrap<'a>);
impl<'a> Deref for Seed<'a> {
    type Target = MemberBootstrap<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Seed<'static> {
    pub fn load(actual_host: &str) -> Option<Self> {
        const REGION: usize = conduit_rp2040_flash_layout::BOOTSTRAP.bytes() as usize;
        // SAFETY: immutable XIP view of the reserved sector, no write access.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                conduit_rp2040_flash_layout::BOOTSTRAP.address() as *const u8,
                REGION,
            )
        };
        if bytes.get(..16)? != MAGIC {
            return None;
        }
        let length = u32::from_le_bytes(bytes.get(16..HEADER)?.try_into().ok()?) as usize;
        if length == 0 || length > REGION - HEADER {
            return None;
        }
        let (seed, used) = serde_json_core::from_slice::<MemberBootstrap<'static>>(
            bytes.get(HEADER..HEADER + length)?,
        )
        .ok()?;
        if used != length
            || !seed.matches_native(
                actual_host,
                crate::config::IMAGE_ID,
                crate::config::CONFIGURATION_ID,
            )
        {
            return None;
        }
        Some(Self(seed))
    }
}

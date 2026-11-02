//! Immutable private Seed, selected by Make before boot. No runtime flash writes.
use alloc::string::String;
use conduit_body::{
    validate_pico_spawn_provision, BodyId, FederationPeer, MembershipCredential, PicoSpawnProvision,
};
use serde::Deserialize;

const MAGIC: &[u8; 16] = b"CONDUIT_SEED@1\0\0";
const HEADER: usize = 20;
pub const SCHEMA: &str = "conduit.body/embedded-membership-seed@1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seed<'a> {
    pub schema: String,
    pub body_id: BodyId,
    pub host_id: String,
    pub configuration_id: String,
    pub owner: FederationPeer,
    #[serde(borrow)]
    pub spawn: PicoSpawnProvision<'a>,
    pub credential: Option<MembershipCredential>,
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
        let (seed, used) =
            serde_json_core::from_slice::<Seed<'static>>(bytes.get(HEADER..HEADER + length)?)
                .ok()?;
        if used != length
            || seed.schema != SCHEMA
            || seed.host_id != actual_host
            || seed.configuration_id != crate::config::CONFIGURATION_ID
            || !validate_pico_spawn_provision(&seed.spawn)
            || seed.spawn.body_id != seed.body_id.as_str()
            || seed.owner.verifying_key == [0; 32]
            || seed.owner.host_id.as_str().is_empty()
            || seed.owner.boot_id.as_str().is_empty()
        {
            return None;
        }
        if let Some(credential) = &seed.credential {
            if credential.body_id != seed.body_id || credential.host_id.as_str() != actual_host {
                return None;
            }
        }
        Some(seed)
    }
}
impl Drop for Seed<'_> {
    fn drop(&mut self) {
        for byte in &mut self.spawn.secret {
            // SAFETY: exclusive initialized private seed bytes.
            unsafe { core::ptr::write_volatile(byte, 0) }
        }
    }
}

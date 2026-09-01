//! Exact Body bootstrap read from the native UF2's reserved final flash sector.

use conduit_body::{
    validate_pico_spawn_provision, PicoSpawnProvision, MAX_PICO_ADMISSION_FRAME_BYTES,
};

const FLASH_BOOTSTRAP_ADDRESS: usize = 0x101f_f000;
const FLASH_BOOTSTRAP_BYTES: usize = 4_096;
const MAGIC: &[u8; 16] = b"CONDUIT_SPORE@1\0";
const HEADER_BYTES: usize = MAGIC.len() + 4;

pub(crate) fn load() -> Option<PicoSpawnProvision<'static>> {
    // SAFETY: RP2040 maps its exact 2 MiB external flash at 0x1000_0000. The
    // fabrication contract reserves and writes the final 4 KiB sector. This
    // immutable view is bounded to that sector and is never written at runtime.
    let bytes = unsafe {
        core::slice::from_raw_parts(
            FLASH_BOOTSTRAP_ADDRESS as *const u8,
            FLASH_BOOTSTRAP_BYTES,
        )
    };
    parse(bytes)
}

fn parse(bytes: &'static [u8]) -> Option<PicoSpawnProvision<'static>> {
    if bytes.len() != FLASH_BOOTSTRAP_BYTES || bytes.get(..MAGIC.len())? != MAGIC {
        return None;
    }
    let length = u32::from_le_bytes(bytes.get(MAGIC.len()..HEADER_BYTES)?.try_into().ok()?) as usize;
    if length == 0
        || length > FLASH_BOOTSTRAP_BYTES - HEADER_BYTES
        || length > MAX_PICO_ADMISSION_FRAME_BYTES
    {
        return None;
    }
    let (provision, used) = serde_json_core::from_slice::<PicoSpawnProvision<'static>>(
        bytes.get(HEADER_BYTES..HEADER_BYTES + length)?,
    )
    .ok()?;
    (used == length && validate_pico_spawn_provision(&provision)).then_some(provision)
}


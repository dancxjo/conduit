//! Explicit native display diagnostic, distinct from complete Mini PiTFT support.
use crate::{arch, identity};

/// Retain the completed physical display Play while the boot Root observes it.
/// Button inputs are reserved but not yet executed by this diagnostic.
///
/// # Safety
/// The selected image Root owns the B+ v1.2 attachment and firmware handoff,
/// including its fixed250MHz core clock and device mappings.
pub unsafe fn run(build_id: &str, stage: fn(u8)) -> Result<impl Sized, &'static str> {
    stage(2);
    let entry =
        super::minipitft_source::entry().map_err(|_| "minipitft-source-preparation-refused")?;
    let nonce = arch::read_counter();
    // These are local boot identities, not cryptographic issuer key material.
    // Native owners obtain capability keys independently from the real RNG.
    let identities = identity::derive(
        [
            nonce,
            nonce.rotate_left(7),
            nonce.rotate_left(19),
            nonce.rotate_left(31),
        ],
        nonce,
        0x8000,
    );
    let host = identity::hex(&identities.host).into();
    let boot = identity::hex(&identities.boot).into();
    stage(3);
    let retained = unsafe { super::minipitft_runtime::run(entry, host, boot, build_id, stage) }?;
    stage(6);
    Ok(retained)
}

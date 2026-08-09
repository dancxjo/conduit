//! Exact-build USB continuity probe shared by physical Pico proofs.

use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcCarrier;

use super::firmware::FirmwareIdentity;
use super::PicoResult;

const BOOTSEL_QUERY: &[u8] = b"CONDUIT_BOOTSEL_QUERY@1";
const BOOTSEL_CHALLENGE_PREFIX: &[u8] = b"CONDUIT_BOOTSEL_CHALLENGE@1:";

pub(super) fn verify(
    carrier: &mut NativePathCdcCarrier,
    identity: &FirmwareIdentity,
) -> PicoResult<()> {
    let mut raw = [0_u8; 2048];
    carrier.send_raw_stream_frame(BOOTSEL_QUERY, Duration::from_secs(2))?;
    let challenge = carrier.receive_raw_stream_frame(&mut raw, Duration::from_secs(3))?;
    let running_build = challenge
        .strip_prefix(BOOTSEL_CHALLENGE_PREFIX)
        .ok_or("USB continuity probe returned an invalid BOOTSEL challenge")?;
    if running_build != identity.firmware_build_id.as_bytes() {
        return Err("USB continuity challenge came from a different firmware build".into());
    }
    Ok(())
}

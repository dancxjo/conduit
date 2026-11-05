//! Existing explicit build-bound developer USB recovery, independent of membership.
use crate::transport::Link;
use core::fmt::Write;
const QUERY: &[u8] = b"CONDUIT_BOOTSEL_QUERY@1";
const CHALLENGE: &str = "CONDUIT_BOOTSEL_CHALLENGE@1:";
const REQUEST: &str = "CONDUIT_REBOOT_BOOTSEL@1:";
const ACK: &[u8] = b"CONDUIT_REBOOT_BOOTSEL_ACK@1";
// Shared by occupied-input ingress: a query fits 64 bytes, but a build-bound
// reboot request also carries the 64-byte build identity.
pub(crate) const CONTROL_FRAME_BYTES: usize = 128;

pub async fn handle(link: &mut Link, request: &[u8]) -> Result<bool, ()> {
    if request == QUERY {
        let mut challenge = heapless::String::<CONTROL_FRAME_BYTES>::new();
        write!(&mut challenge, "{CHALLENGE}{}", crate::BUILD_ID).map_err(|_| ())?;
        link.send(challenge.as_bytes()).await?;
        return Ok(true);
    }
    let mut expected = heapless::String::<CONTROL_FRAME_BYTES>::new();
    write!(&mut expected, "{REQUEST}{}", crate::BUILD_ID).map_err(|_| ())?;
    if request != expected.as_bytes() {
        return Ok(false);
    }
    link.send(ACK).await?;
    embassy_time::Timer::after_millis(100).await;
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    core::future::pending().await
}

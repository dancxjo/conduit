//! Explicit build-bound developer control for rebooting into RP2040 BOOTSEL.

use core::fmt::Write as _;
use embassy_time::{Duration, Timer};
use heapless::String;

#[cfg(feature = "distributed-lenia")]
use crate::lenia_image::FIRMWARE_BUILD_ID;
#[cfg(feature = "wifi-bootstrap")]
use crate::network_image::FIRMWARE_BUILD_ID;
#[cfg(all(
    not(feature = "wifi-bootstrap"),
    not(feature = "appliance-hello"),
    not(feature = "appliance-hil-client")
))]
#[cfg(not(feature = "distributed-lenia"))]
use crate::signal_image::FIRMWARE_BUILD_ID;
#[cfg(feature = "appliance-hello")]
const FIRMWARE_BUILD_ID: &str = env!("CONDUIT_PICO_APPLIANCE_BUILD_ID");
#[cfg(feature = "appliance-hil-client")]
const FIRMWARE_BUILD_ID: &str = env!("CONDUIT_PICO_APPLIANCE_BUILD_ID");
use crate::usb_link::{UsbLinkError, UsbLinkSession};

pub const QUERY: &[u8] = b"CONDUIT_BOOTSEL_QUERY@1";
pub const CHALLENGE_PREFIX: &str = "CONDUIT_BOOTSEL_CHALLENGE@1:";
pub const REQUEST_PREFIX: &str = "CONDUIT_REBOOT_BOOTSEL@1:";
pub const ACK: &[u8] = b"CONDUIT_REBOOT_BOOTSEL_ACK@1";

/// Handle one raw CDC 0 control frame. Returns `true` when the frame belongs
/// to the BOOTSEL protocol, allowing callers that also own a session protocol
/// to keep control traffic out of its decoder.
pub async fn handle_request(
    link: &mut UsbLinkSession,
    request: &[u8],
) -> Result<bool, UsbLinkError> {
    if request == QUERY {
        let mut challenge = String::<1024>::new();
        if write!(challenge, "{CHALLENGE_PREFIX}{FIRMWARE_BUILD_ID}").is_err() {
            return Err(UsbLinkError::BufferOverflow);
        }
        link.send_raw_stream_frame(challenge.as_bytes()).await?;
        return Ok(true);
    }

    let mut expected = String::<1024>::new();
    if write!(expected, "{REQUEST_PREFIX}{FIRMWARE_BUILD_ID}").is_err() {
        return Err(UsbLinkError::BufferOverflow);
    }
    if request != expected.as_bytes() {
        return Ok(false);
    }

    link.send_raw_stream_frame(ACK).await?;
    Timer::after(Duration::from_millis(100)).await;
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    core::future::pending::<Result<bool, UsbLinkError>>().await
}

pub async fn wait_for_request(link: &mut UsbLinkSession) -> Result<(), UsbLinkError> {
    let mut input = [0_u8; 1024];

    // A CDC owner such as ModemManager may have written unrelated startup
    // bytes before the explicit Conduit client opens the endpoint. Never let
    // those bytes poison every later authenticated recovery attempt.
    link.reset_stream_decoder();
    link.wait_connection().await;
    loop {
        let request = match link.receive_control_stream_frame(&mut input).await {
            Ok(request) => request,
            Err(error) => {
                link.reset_stream_decoder();
                return Err(error);
            }
        };
        handle_request(link, request).await?;
    }
}

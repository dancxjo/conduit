use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::serial::resolve_dual_ports;
use super::{PicoArgs, PicoResult};

const QUERY: &[u8] = b"CONDUIT_BOOTSEL_QUERY@1";
const CHALLENGE_PREFIX: &[u8] = b"CONDUIT_BOOTSEL_CHALLENGE@1:";
const REQUEST_PREFIX: &str = "CONDUIT_REBOOT_BOOTSEL@1:";
const ACK: &[u8] = b"CONDUIT_REBOOT_BOOTSEL_ACK@1";

pub fn run_bootsel(args: &PicoArgs) -> PicoResult<()> {
    if args.dry_run {
        println!("==> pico bootsel (dry-run): request exact current firmware over CDC 0");
        return Ok(());
    }

    let (link_port, _) = resolve_dual_ports(args.link_port.as_deref(), args.port.as_deref())?;
    println!(
        "==> pico bootsel: requesting reboot from {}",
        link_port.display()
    );
    let mut line = NativePathCdcLine::open(&link_port, 1024)?;
    std::thread::sleep(Duration::from_millis(250));
    line.send_raw_stream_frame(QUERY, Duration::from_secs(2))?;
    let mut challenge = [0_u8; 1024];
    let challenge = line.receive_raw_stream_frame(&mut challenge, Duration::from_secs(2))?;
    let running_build = challenge
        .strip_prefix(CHALLENGE_PREFIX)
        .ok_or_else(|| format!("unexpected Pico BOOTSEL challenge: {challenge:?}"))?;
    let running_build = std::str::from_utf8(running_build)
        .map_err(|_| "Pico BOOTSEL challenge contains a non-UTF-8 build identity")?;
    println!("==> pico bootsel: running build {running_build}");
    let request = format!("{REQUEST_PREFIX}{running_build}");
    line.send_raw_stream_frame(request.as_bytes(), Duration::from_secs(2))?;
    let mut response = [0_u8; 128];
    let response = line.receive_raw_stream_frame(&mut response, Duration::from_secs(2))?;
    if response != ACK {
        return Err(format!("unexpected Pico BOOTSEL acknowledgement: {response:?}").into());
    }
    println!("==> pico bootsel: exact build acknowledged reboot request");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_bound_to_the_exact_firmware_build() {
        let first = format!("{REQUEST_PREFIX}build-a");
        let second = format!("{REQUEST_PREFIX}build-b");
        assert_ne!(first, second);
        assert_eq!(QUERY, b"CONDUIT_BOOTSEL_QUERY@1");
        assert_eq!(CHALLENGE_PREFIX, b"CONDUIT_BOOTSEL_CHALLENGE@1:");
        assert_eq!(ACK, b"CONDUIT_REBOOT_BOOTSEL_ACK@1");
    }
}

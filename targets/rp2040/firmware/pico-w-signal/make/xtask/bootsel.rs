use std::time::Duration;

use conduit_std_host::usb_cdc::NativePathCdcLine;

use super::capstone_serial::resolve_port as resolve_capstone_port;
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

    let link_port = if args.pete_capstone {
        resolve_capstone_port(args.link_port.as_deref().or(args.port.as_deref()))?
    } else {
        resolve_dual_ports(args.link_port.as_deref(), args.port.as_deref())?.0
    };
    println!(
        "==> pico bootsel: requesting reboot from {}",
        link_port.display()
    );
    let (mut line, challenge, challenge_len) = query_running_build(&link_port)?;
    let challenge = &challenge[..challenge_len];
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

fn query_running_build(
    link_port: &std::path::Path,
) -> PicoResult<(NativePathCdcLine, [u8; 1024], usize)> {
    let mut first_error = None;
    for attempt in 0..2 {
        let mut line = NativePathCdcLine::open(link_port, 1024)?;
        std::thread::sleep(Duration::from_millis(250));
        line.send_raw_stream_frame(QUERY, Duration::from_secs(2))?;
        let mut challenge = [0_u8; 1024];
        match line.receive_raw_stream_frame(&mut challenge, Duration::from_secs(2)) {
            Ok(received) => {
                let length = received.len();
                return Ok((line, challenge, length));
            }
            Err(error) if attempt == 0 => {
                println!(
                    "==> pico bootsel: discarded unread startup bytes before control framing ({error})"
                );
                first_error = Some(error);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(first_error
        .expect("two-attempt loop records its first failure")
        .into())
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

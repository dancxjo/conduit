//! Shared exact std/Create device-session and evidence publication helpers.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use conduit_create_oi::{
    encode_mode, encode_pause_stream, encode_sensor_stream, encode_start, read_stream_packet,
    write_command, CreateOiFailure, CreateOiModeRequest, CreateOiPacket, CreateUartProvider,
};
use conduit_pete::OiMode;
use conduit_std_host::std_create_uart::monotonic_millis;

const OI_MODE_PACKET: u8 = 35;
const CREATE_1_FULL_ACQUISITION_ATTEMPTS: u8 = 10;

pub(crate) fn establish_safe<P: CreateUartProvider>(
    provider: &mut P,
    read_timeout_ms: u32,
) -> Result<OiMode, String> {
    establish_mode(provider, read_timeout_ms, CreateOiModeRequest::Safe, 1)
}

pub(crate) fn establish_full<P: CreateUartProvider>(
    provider: &mut P,
    read_timeout_ms: u32,
) -> Result<OiMode, String> {
    establish_mode(
        provider,
        read_timeout_ms,
        CreateOiModeRequest::Full,
        CREATE_1_FULL_ACQUISITION_ATTEMPTS,
    )
}

fn establish_mode<P: CreateUartProvider>(
    provider: &mut P,
    read_timeout_ms: u32,
    requested: CreateOiModeRequest,
    attempts: u8,
) -> Result<OiMode, String> {
    let expected = match requested {
        CreateOiModeRequest::Passive => OiMode::Passive,
        CreateOiModeRequest::Safe => OiMode::Safe,
        CreateOiModeRequest::Full => OiMode::Full,
    };
    let mut last_failure = "no acquisition attempt completed".to_string();
    for attempt in 1..=attempts {
        write_command(provider, &encode_start()).map_err(protocol_error)?;
        write_command(
            provider,
            &encode_mode(requested).expect("SAFE and FULL each have one exact command"),
        )
        .map_err(protocol_error)?;
        let deadline = monotonic_millis()
            .map_err(|error| format!("monotonic clock: {error:?}"))?
            .checked_add(u64::from(read_timeout_ms))
            .ok_or_else(|| "mode deadline overflow".to_string())?;
        match read_framed_status(provider, OI_MODE_PACKET, deadline) {
            Ok(packet) => {
                let mode = match packet.bytes()[0] {
                    0 => OiMode::Off,
                    1 => OiMode::Passive,
                    2 => OiMode::Safe,
                    3 => OiMode::Full,
                    _ => return Err("invalid OI mode payload".into()),
                };
                if mode == expected {
                    return Ok(mode);
                }
                last_failure =
                    format!("attempt {attempt} observed {mode:?} after {requested:?} request");
            }
            Err(CreateStatusFailure {
                primary: Some(CreateOiFailure::DeviceNoResponse),
                pause_failure: None,
            }) => {
                last_failure = format!("attempt {attempt} received no mode response");
            }
            Err(error) => return Err(format!("mode query: {error:?}")),
        }
    }
    Err(format!(
        "failed to establish {requested:?} after {attempts} attempts: {last_failure}"
    ))
}

/// Failure of a single framed status request, retaining pause cleanup separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CreateStatusFailure {
    pub(crate) primary: Option<CreateOiFailure>,
    pub(crate) pause_failure: Option<CreateOiFailure>,
}

/// Read one strict packet-35/36/37 sample and request stream pause before use.
///
/// The existing reader uses fixed maximum frame storage and consumes exactly
/// five bytes for these one-byte packets (or stops on incomplete/provider data).
/// Unexpected bytes refuse without scanning, draining, or raw-query fallback.
/// A valid sample has no transaction nonce and does not establish freshness or
/// delivery of the pause command. The caller owns the one absolute deadline.
pub(crate) fn read_framed_status<P: CreateUartProvider>(
    provider: &mut P,
    packet_id: u8,
    deadline_tick: u64,
) -> Result<CreateOiPacket, CreateStatusFailure> {
    if !(35..=37).contains(&packet_id) {
        return Err(CreateStatusFailure {
            primary: Some(CreateOiFailure::UnsupportedPacket(packet_id)),
            pause_failure: None,
        });
    }
    let command = encode_sensor_stream(packet_id).expect("status packets are allow-listed");
    // A failed write can have delivered a prefix: attempt pause even then.
    let primary = write_command(provider, &command)
        .and_then(|()| read_stream_packet(provider, packet_id, deadline_tick));
    let pause = write_command(provider, &encode_pause_stream());
    match (primary, pause) {
        (Ok(packet), Ok(())) => Ok(packet),
        (primary, pause) => Err(CreateStatusFailure {
            primary: primary.err(),
            pause_failure: pause.err(),
        }),
    }
}

pub(crate) fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "evidence destination already exists: {}",
            path.display()
        ));
    }
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(".tmp-{}", std::process::id()));
    let temporary = PathBuf::from(temporary);
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        std::fs::hard_link(&temporary, path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

fn protocol_error(error: CreateOiFailure) -> String {
    format!("Create OI protocol: {error:?}")
}

#[cfg(test)]
#[path = "pete_std_create/tests.rs"]
mod tests;

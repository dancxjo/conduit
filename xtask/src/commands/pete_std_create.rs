//! Shared exact std/Create device-session and evidence publication helpers.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use conduit_create_oi::{
    encode_mode, encode_query_sensor, encode_start, read_query_sensor_packet, write_command,
    CreateOiFailure, CreateOiModeRequest,
};
use conduit_pete::OiMode;
use conduit_std_host::std_create_uart::{monotonic_millis, StdCreateUartBase};

const OI_MODE_PACKET: u8 = 35;

pub(crate) fn establish_safe(
    provider: &mut StdCreateUartBase,
    read_timeout_ms: u32,
) -> Result<OiMode, String> {
    establish_mode(provider, read_timeout_ms, CreateOiModeRequest::Safe)
}

pub(crate) fn establish_full(
    provider: &mut StdCreateUartBase,
    read_timeout_ms: u32,
) -> Result<OiMode, String> {
    establish_mode(provider, read_timeout_ms, CreateOiModeRequest::Full)
}

fn establish_mode(
    provider: &mut StdCreateUartBase,
    read_timeout_ms: u32,
    requested: CreateOiModeRequest,
) -> Result<OiMode, String> {
    write_command(provider, &encode_start()).map_err(protocol_error)?;
    write_command(
        provider,
        &encode_mode(requested).expect("SAFE and FULL each have one exact command"),
    )
    .map_err(protocol_error)?;
    let query = encode_query_sensor(OI_MODE_PACKET).map_err(protocol_error)?;
    write_command(provider, &query).map_err(protocol_error)?;
    let deadline = monotonic_millis()
        .map_err(|error| format!("monotonic clock: {error:?}"))?
        .checked_add(u64::from(read_timeout_ms))
        .ok_or_else(|| "mode deadline overflow".to_string())?;
    let packet = read_query_sensor_packet(provider, OI_MODE_PACKET, deadline)
        .map_err(|error| format!("mode query: {error:?}"))?;
    let mode = match packet.bytes()[0] {
        0 => OiMode::Off,
        1 => OiMode::Passive,
        2 => OiMode::Safe,
        3 => OiMode::Full,
        _ => return Err("invalid OI mode payload".into()),
    };
    let expected = match requested {
        CreateOiModeRequest::Passive => OiMode::Passive,
        CreateOiModeRequest::Safe => OiMode::Safe,
        CreateOiModeRequest::Full => OiMode::Full,
    };
    if mode != expected {
        return Err(format!(
            "device mode after {requested:?} request was {mode:?}"
        ));
    }
    Ok(mode)
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

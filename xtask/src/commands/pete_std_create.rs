//! Shared exact std/Create device-session and evidence publication helpers.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use conduit_create_oi::{
    encode_mode, encode_query_sensor, encode_start, read_query_sensor_packet, write_command,
    CreateOiFailure, CreateOiModeRequest, CreateUartProvider,
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
        let query = encode_query_sensor(OI_MODE_PACKET).map_err(protocol_error)?;
        write_command(provider, &query).map_err(protocol_error)?;
        let deadline = monotonic_millis()
            .map_err(|error| format!("monotonic clock: {error:?}"))?
            .checked_add(u64::from(read_timeout_ms))
            .ok_or_else(|| "mode deadline overflow".to_string())?;
        match read_query_sensor_packet(provider, OI_MODE_PACKET, deadline) {
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
            Err(CreateOiFailure::DeviceNoResponse) => {
                last_failure = format!("attempt {attempt} received no mode response");
            }
            Err(error) => return Err(format!("mode query: {error:?}")),
        }
    }
    Err(format!(
        "failed to establish {requested:?} after {attempts} attempts: {last_failure}"
    ))
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
mod tests {
    use super::*;
    use conduit_create_oi::UartProfile;
    use std::collections::VecDeque;

    struct Provider {
        writes: Vec<Vec<u8>>,
        reads: VecDeque<Option<u8>>,
    }

    impl CreateUartProvider for Provider {
        type Error = ();

        fn is_available(&self) -> bool {
            true
        }

        fn profile(&self) -> UartProfile {
            UartProfile::CREATE_OI
        }

        fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
            self.writes.push(bytes.to_vec());
            Ok(())
        }

        fn read_byte(&mut self, _: u64) -> Result<Option<u8>, Self::Error> {
            Ok(self.reads.pop_front().flatten())
        }
    }

    #[test]
    fn full_acquisition_retries_start_and_full_until_confirmed() {
        let mut provider = Provider {
            writes: Vec::new(),
            reads: VecDeque::from([None, Some(2), Some(3)]),
        };

        assert_eq!(establish_full(&mut provider, 1).unwrap(), OiMode::Full);
        assert_eq!(
            provider.writes,
            [
                vec![128],
                vec![132],
                vec![142, 35],
                vec![128],
                vec![132],
                vec![142, 35],
                vec![128],
                vec![132],
                vec![142, 35],
            ]
        );
    }

    #[test]
    fn safe_cleanup_remains_one_exact_non_retrying_transaction() {
        let mut provider = Provider {
            writes: Vec::new(),
            reads: VecDeque::from([Some(2)]),
        };

        assert_eq!(establish_safe(&mut provider, 1).unwrap(), OiMode::Safe);
        assert_eq!(provider.writes, [vec![128], vec![131], vec![142, 35]]);
    }
}

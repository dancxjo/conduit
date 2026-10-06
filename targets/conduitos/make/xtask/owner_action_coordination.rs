//! Bounded harness rendezvous while a real QMP guest and browser remain live.

use std::{
    fs,
    path::Path,
    process::Child,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::{refusal, wait_or_refuse, ConduitosError};

pub(super) fn write_checkpoint(
    directory: &Path,
    name: &str,
    record: &Value,
) -> Result<(), ConduitosError> {
    let temporary = directory.join(format!("{name}.tmp"));
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(record).map_err(|error| {
            ConduitosError::refusal("native-owner-proof-encode", error.to_string())
        })?,
    )
    .map_err(|error| ConduitosError::refusal("native-owner-proof-artifact", error.to_string()))?;
    fs::rename(temporary, directory.join(name))
        .map_err(|error| ConduitosError::refusal("native-owner-proof-artifact", error.to_string()))
}

pub(super) fn wait_for_resume(
    directory: &Path,
    name: &str,
    child: &mut Child,
    timeout: Duration,
) -> Result<(), ConduitosError> {
    let marker = directory.join(name);
    let deadline = Instant::now() + timeout;
    loop {
        match fs::metadata(&marker) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= 16 => {
                if fs::read(&marker).map_err(|error| {
                    ConduitosError::refusal("native-owner-proof-coordination", error.to_string())
                })? == b"continue\n"
                {
                    return Ok(());
                }
                return Err(refusal("native-owner-proof-coordination-invalid"));
            }
            Ok(_) => return Err(refusal("native-owner-proof-coordination-invalid")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ConduitosError::refusal(
                    "native-owner-proof-coordination",
                    error.to_string(),
                ));
            }
        }
        wait_or_refuse(child, deadline, "native-owner-proof-coordination-timeout")?;
    }
}

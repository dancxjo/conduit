//! Bounded guest stack observation after an already failed journey.
use std::{fs, os::unix::net::UnixStream, path::Path};

use serde_json::{json, Value};

use super::super::{qmp, ConduitosError};

pub(super) fn capture(
    stream: &mut UnixStream,
    reader: &mut qmp::Reader,
    registers: &Value,
    target: &Path,
) -> Result<(), ConduitosError> {
    let rsp = stack_pointer(registers).ok_or_else(|| {
        ConduitosError::refusal("journey-failure-stack", "missing x86_64 stack pointer")
    })?;
    // HMP x reads guest virtual memory, unlike xp (physical memory).
    // A fixed 64 words preserves candidate return addresses without a RAM dump.
    let command = json!({"execute":"human-monitor-command", "arguments": {
        "command-line": format!("x /64gx 0x{rsp:016x}")
    }});
    let stack = qmp::request_value(
        stream,
        reader,
        &serde_json::to_vec(&command).map_err(io_error)?,
        "journey-failure-stack",
    )?;
    let record = json!({
        "schema":"conduit.conduitos/journey-failure-stack@1",
        "address_space":"guest-virtual", "rsp":format!("{rsp:016x}"),
        "word_count":64, "word_bytes":8, "hmp_result":stack,
    });
    fs::write(
        target.join("journey-failure-stack.json"),
        serde_json::to_vec_pretty(&record).map_err(io_error)?,
    )
    .map_err(io_error)
}

fn stack_pointer(registers: &Value) -> Option<u64> {
    registers.as_str()?.split_whitespace().find_map(|field| {
        let value = field.strip_prefix("RSP=")?;
        (value.len() == 16)
            .then(|| u64::from_str_radix(value, 16).ok())
            .flatten()
    })
}

fn io_error(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("journey-failure-stack", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_a_complete_named_stack_pointer() {
        assert_eq!(
            stack_pointer(&json!(
                "RBP=000000006cccfec7 RSP=ffff8000020c3320\r\nRIP=ffffffff809cf003"
            )),
            Some(0xffff8000020c3320)
        );
        for value in [
            json!(null),
            json!("RSP=bad"),
            json!("RSP=gggggggggggggggg"),
            json!("XRSP=ffff8000020c3320"),
        ] {
            assert_eq!(stack_pointer(&value), None);
        }
    }
}

//! Bounded QMP key injection synchronized to admitted guest HID progress.

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
    process::Child,
    thread,
    time::{Duration, Instant},
};

use super::ConduitosError;

pub(super) fn inject(
    socket: &Path,
    serial_path: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut qmp = loop {
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Err(error) => return stop(child, "qemu-qmp-unavailable", error.to_string()),
        }
    };
    qmp.set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?;
    let mut reader = BufReader::new(
        qmp.try_clone()
            .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?,
    );
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?;
    if !response.contains("\"QMP\"") {
        return Err(ConduitosError::refusal(
            "qemu-qmp-failed",
            format!("missing QMP greeting: {response}"),
        ));
    }
    qmp.write_all(b"{\"execute\":\"qmp_capabilities\"}\r\n")
        .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?;
    require_return(&mut reader, "capability negotiation")?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-awaiting-qemu-key",
        "hid-ready-timeout",
    )?;
    send_key(&mut qmp, &mut reader, true)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-press-report",
        "hid-press-timeout",
    )?;
    send_key(&mut qmp, &mut reader, false)
}

fn send_key(
    qmp: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
    down: bool,
) -> Result<(), ConduitosError> {
    let command = if down {
        b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"a\"}}}]}}\r\n"
            .as_slice()
    } else {
        b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":false,\"key\":{\"type\":\"qcode\",\"data\":\"a\"}}}]}}\r\n"
            .as_slice()
    };
    qmp.write_all(command)
        .map_err(|error| ConduitosError::refusal("qemu-key-injection-failed", error.to_string()))?;
    require_return(reader, if down { "key-down" } else { "key-up" })
}

fn wait_for_stage(
    serial_path: &Path,
    child: &mut Child,
    stage: &str,
    reason: &'static str,
) -> Result<(), ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if fs::read_to_string(serial_path).is_ok_and(|serial| serial.contains(stage)) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return stop(child, reason, format!("guest did not emit {stage}"));
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn require_return(
    reader: &mut BufReader<UnixStream>,
    action: &'static str,
) -> Result<(), ConduitosError> {
    for _ in 0..8 {
        let mut response = String::new();
        reader
            .read_line(&mut response)
            .map_err(|error| ConduitosError::refusal("qemu-qmp-failed", error.to_string()))?;
        if response.contains("\"return\"") {
            return Ok(());
        }
        if response.contains("\"error\"") || response.is_empty() {
            return Err(ConduitosError::refusal(
                "qemu-key-injection-failed",
                format!("QMP {action} response: {response}"),
            ));
        }
    }
    Err(ConduitosError::refusal(
        "qemu-key-injection-failed",
        format!("QMP {action} produced no bounded response"),
    ))
}

fn stop<T>(child: &mut Child, reason: &'static str, detail: String) -> Result<T, ConduitosError> {
    let _ = child.kill();
    let _ = child.wait();
    Err(ConduitosError::refusal(reason, detail))
}

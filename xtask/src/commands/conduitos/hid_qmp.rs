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

#[derive(Clone, Copy)]
pub(super) enum RescueNearMiss {
    ControlDelete,
    AltDelete,
    ControlAltBackspace,
}

pub(super) fn inject(
    socket: &Path,
    serial_path: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    let (mut qmp, mut reader) = connect(socket, child)?;
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

pub(super) fn inject_rescue_and_wait_for_reboot(
    socket: &Path,
    serial_path: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    let (mut qmp, mut reader) = connect(socket, child)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-start",
        "rescue-hid-start-timeout",
    )?;
    send_rescue_modifiers(&mut qmp, &mut reader)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-press-report",
        "rescue-report-timeout",
    )?;
    send_rescue_delete(&mut qmp, &mut reader)?;
    wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-release-report",
        3,
        "rescue-delete-report-timeout",
    )?;
    send_rescue_keys(&mut qmp, &mut reader, false)?;
    wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE local-rescue-ready",
        2,
        "rescue-reboot-timeout",
    )
}

pub(super) fn inject_near_miss(
    socket: &Path,
    serial_path: &Path,
    child: &mut Child,
    case: RescueNearMiss,
) -> Result<(), ConduitosError> {
    let (mut qmp, mut reader) = connect(socket, child)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-start",
        "rescue-negative-hid-start-timeout",
    )?;
    let modifiers = match case {
        RescueNearMiss::ControlDelete => &["ctrl"][..],
        RescueNearMiss::AltDelete => &["alt"][..],
        RescueNearMiss::ControlAltBackspace => &["ctrl", "alt"][..],
    };
    send_named_keys(
        &mut qmp,
        &mut reader,
        modifiers,
        true,
        "near-miss-modifiers",
    )?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-press-report",
        "rescue-negative-prefix-timeout",
    )?;
    let key = match case {
        RescueNearMiss::ControlDelete | RescueNearMiss::AltDelete => "delete",
        RescueNearMiss::ControlAltBackspace => "backspace",
    };
    send_named_keys(&mut qmp, &mut reader, &[key], true, "near-miss-key")?;
    wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-release-report",
        1,
        "rescue-negative-report-timeout",
    )?;
    send_named_keys(
        &mut qmp,
        &mut reader,
        &["ctrl", "alt", key],
        false,
        "near-miss-release",
    )?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_KERNEL_SIGN",
        "rescue-negative-terminal-timeout",
    )
}

fn connect(
    socket: &Path,
    child: &mut Child,
) -> Result<(UnixStream, BufReader<UnixStream>), ConduitosError> {
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
    Ok((qmp, reader))
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

fn send_rescue_keys(
    qmp: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
    down: bool,
) -> Result<(), ConduitosError> {
    let state = if down { "true" } else { "false" };
    let command = format!(
        "{{\"execute\":\"input-send-event\",\"arguments\":{{\"events\":[{{\"type\":\"key\",\"data\":{{\"down\":{state},\"key\":{{\"type\":\"qcode\",\"data\":\"ctrl\"}}}}}},{{\"type\":\"key\",\"data\":{{\"down\":{state},\"key\":{{\"type\":\"qcode\",\"data\":\"alt\"}}}}}},{{\"type\":\"key\",\"data\":{{\"down\":{state},\"key\":{{\"type\":\"qcode\",\"data\":\"delete\"}}}}}}]}}}}\r\n"
    );
    qmp.write_all(command.as_bytes())
        .map_err(|error| ConduitosError::refusal("qemu-key-injection-failed", error.to_string()))?;
    require_return(
        reader,
        if down {
            "rescue-key-down"
        } else {
            "rescue-key-up"
        },
    )
}

fn send_rescue_modifiers(
    qmp: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
) -> Result<(), ConduitosError> {
    let command = b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"ctrl\"}}},{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"alt\"}}}]}}\r\n";
    qmp.write_all(command)
        .map_err(|error| ConduitosError::refusal("qemu-key-injection-failed", error.to_string()))?;
    require_return(reader, "rescue-modifiers-down")
}

fn send_rescue_delete(
    qmp: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
) -> Result<(), ConduitosError> {
    let command = b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"delete\"}}}]}}\r\n";
    qmp.write_all(command)
        .map_err(|error| ConduitosError::refusal("qemu-key-injection-failed", error.to_string()))?;
    require_return(reader, "rescue-delete-down")
}

fn send_named_keys(
    qmp: &mut UnixStream,
    reader: &mut BufReader<UnixStream>,
    keys: &[&str],
    down: bool,
    action: &'static str,
) -> Result<(), ConduitosError> {
    let mut events = String::new();
    for (index, key) in keys.iter().enumerate() {
        if index != 0 {
            events.push(',');
        }
        events.push_str(&format!(
            "{{\"type\":\"key\",\"data\":{{\"down\":{down},\"key\":{{\"type\":\"qcode\",\"data\":\"{key}\"}}}}}}"
        ));
    }
    let command =
        format!("{{\"execute\":\"input-send-event\",\"arguments\":{{\"events\":[{events}]}}}}\r\n");
    qmp.write_all(command.as_bytes())
        .map_err(|error| ConduitosError::refusal("qemu-key-injection-failed", error.to_string()))?;
    require_return(reader, action)
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

fn wait_for_stage_count(
    serial_path: &Path,
    child: &mut Child,
    stage: &str,
    count: usize,
    reason: &'static str,
) -> Result<(), ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if fs::read_to_string(serial_path)
            .is_ok_and(|serial| serial.matches(stage).count() >= count)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return stop(
                child,
                reason,
                format!("guest did not emit {count} occurrences of {stage}"),
            );
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

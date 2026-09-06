//! Bounded QMP key injection synchronized to admitted guest HID progress.

use std::{
    fs,
    os::unix::net::UnixStream,
    path::Path,
    process::Child,
    thread,
    time::{Duration, Instant},
};

pub(super) use super::qmp::connect;
use super::{qmp, ConduitosError};

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
    send_key(&mut qmp, &mut reader, false)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE keyboard-text-play-started",
        "keyboard-text-start-timeout",
    )?;
    inject_keyboard_text(&mut qmp, &mut reader, serial_path, child)
}

fn inject_keyboard_text(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
    serial_path: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    const EVENTS: [(&str, bool); 38] = [
        ("h", true),
        ("h", false),
        ("e", true),
        ("e", false),
        ("l", true),
        ("l", false),
        ("l", true),
        ("l", false),
        ("o", true),
        ("o", false),
        ("alt_r", true),
        ("a", true),
        ("a", false),
        ("alt_r", false),
        ("alt_r", true),
        ("spc", true),
        ("spc", false),
        ("alt_r", false),
        ("apostrophe", true),
        ("apostrophe", false),
        ("e", true),
        ("e", false),
        ("alt_r", true),
        ("shift", true),
        ("spc", true),
        ("spc", false),
        ("shift", false),
        ("alt_r", false),
        ("0", true),
        ("0", false),
        ("3", true),
        ("3", false),
        ("b", true),
        ("b", false),
        ("b", true),
        ("b", false),
        ("ret", true),
        ("ret", false),
    ];
    for (index, (key, down)) in EVENTS.iter().copied().enumerate() {
        wait_for_stage_count(
            serial_path,
            child,
            "CONDUIT_BOOT_STAGE hid-awaiting-followup-report",
            index + 1,
            "keyboard-text-ready-timeout",
        )?;
        send_named_keys(qmp, reader, &[key], down, "keyboard-text-key")?;
        wait_for_stage_count(
            serial_path,
            child,
            "CONDUIT_BOOT_STAGE hid-release-report",
            index + 2,
            "keyboard-text-report-timeout",
        )?;
    }
    Ok(())
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

pub(super) fn inject_active_play_rescue_and_wait_for_reboot(
    socket: &Path,
    serial_path: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    let (mut qmp, mut reader) = connect(socket, child)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-awaiting-qemu-key",
        "active-rescue-hid-ready-timeout",
    )?;
    send_key(&mut qmp, &mut reader, true)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-press-report",
        "active-rescue-press-timeout",
    )?;
    send_key(&mut qmp, &mut reader, false)?;
    wait_for_stage(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE keyboard-text-play-started",
        "active-rescue-play-timeout",
    )?;
    send_rescue_modifiers(&mut qmp, &mut reader)?;
    wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE hid-release-report",
        2,
        "active-rescue-modifiers-timeout",
    )?;
    send_rescue_delete(&mut qmp, &mut reader)?;
    wait_for_stage_count(
        serial_path,
        child,
        "CONDUIT_BOOT_STAGE local-rescue-ready",
        2,
        "active-rescue-reboot-timeout",
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

fn send_key(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
    down: bool,
) -> Result<(), ConduitosError> {
    let command = if down {
        b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"a\"}}}]}}\r\n"
            .as_slice()
    } else {
        b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":false,\"key\":{\"type\":\"qcode\",\"data\":\"a\"}}}]}}\r\n"
            .as_slice()
    };
    qmp::request(
        qmp,
        reader,
        command,
        if down { "key-down" } else { "key-up" },
    )
}

fn send_rescue_keys(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
    down: bool,
) -> Result<(), ConduitosError> {
    let state = if down { "true" } else { "false" };
    let command = format!(
        "{{\"execute\":\"input-send-event\",\"arguments\":{{\"events\":[{{\"type\":\"key\",\"data\":{{\"down\":{state},\"key\":{{\"type\":\"qcode\",\"data\":\"ctrl\"}}}}}},{{\"type\":\"key\",\"data\":{{\"down\":{state},\"key\":{{\"type\":\"qcode\",\"data\":\"alt\"}}}}}},{{\"type\":\"key\",\"data\":{{\"down\":{state},\"key\":{{\"type\":\"qcode\",\"data\":\"delete\"}}}}}}]}}}}\r\n"
    );
    qmp::request(
        qmp,
        reader,
        command.as_bytes(),
        if down {
            "rescue-key-down"
        } else {
            "rescue-key-up"
        },
    )
}

fn send_rescue_modifiers(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
) -> Result<(), ConduitosError> {
    let command = b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"ctrl\"}}},{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"alt\"}}}]}}\r\n";
    qmp::request(qmp, reader, command.as_ref(), "rescue-modifiers-down")
}

fn send_rescue_delete(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
) -> Result<(), ConduitosError> {
    let command = b"{\"execute\":\"input-send-event\",\"arguments\":{\"events\":[{\"type\":\"key\",\"data\":{\"down\":true,\"key\":{\"type\":\"qcode\",\"data\":\"delete\"}}}]}}\r\n";
    qmp::request(qmp, reader, command.as_ref(), "rescue-delete-down")
}

pub(super) fn send_named_keys(
    qmp: &mut UnixStream,
    reader: &mut super::qmp::Reader,
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
    qmp::request(qmp, reader, command.as_ref(), action)
}

pub(super) fn wait_for_stage(
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

pub(super) fn stop<T>(
    child: &mut Child,
    reason: &'static str,
    detail: String,
) -> Result<T, ConduitosError> {
    let _ = child.kill();
    let _ = child.wait();
    Err(ConduitosError::refusal(reason, detail))
}

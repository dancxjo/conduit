//! Exact QMP choreography for the bounded K7 detach/reattach proof.

use std::{path::Path, process::Child};

use super::{hid_qmp, ConduitosError};

pub(super) fn execute(
    socket: &Path,
    serial: &Path,
    child: &mut Child,
) -> Result<(), ConduitosError> {
    hid_qmp::inject(socket, serial, child)?;
    let (mut qmp, mut reader) = hid_qmp::connect(socket, child)?;
    hid_qmp::wait_for_stage(
        serial,
        child,
        "CONDUIT_BOOT_STAGE hotplug-d1-key-down",
        "hotplug-d1-timeout",
    )?;
    hid_qmp::send_named_keys(&mut qmp, &mut reader, &["c"], true, "d1-key-down")?;
    hid_qmp::wait_for_stage(
        serial,
        child,
        "CONDUIT_BOOT_STAGE hotplug-d1-key-up",
        "hotplug-d1-release-timeout",
    )?;
    hid_qmp::send_named_keys(&mut qmp, &mut reader, &["c"], false, "d1-key-up")?;
    hid_qmp::wait_for_stage(
        serial,
        child,
        "CONDUIT_BOOT_STAGE hotplug-d1-transfer-pending",
        "hotplug-pending-timeout",
    )?;
    command(
        &mut qmp,
        &mut reader,
        b"{\"execute\":\"device_del\",\"arguments\":{\"id\":\"keyboard-d1\"}}\r\n",
        "device-del-d1",
    )?;
    hid_qmp::wait_for_stage(
        serial,
        child,
        "CONDUIT_BOOT_STAGE hotplug-d1-retired",
        "hotplug-retirement-timeout",
    )?;
    command(&mut qmp, &mut reader,
        b"{\"execute\":\"device_add\",\"arguments\":{\"driver\":\"usb-kbd\",\"id\":\"keyboard-d2\",\"bus\":\"conduitos-xhci.0\",\"port\":\"1\"}}\r\n",
        "device-add-d2")?;
    hid_qmp::wait_for_stage(
        serial,
        child,
        "CONDUIT_BOOT_STAGE hotplug-d2-key-down",
        "hotplug-d2-timeout",
    )?;
    hid_qmp::send_named_keys(&mut qmp, &mut reader, &["d"], true, "d2-key-down")?;
    hid_qmp::wait_for_stage(
        serial,
        child,
        "CONDUIT_BOOT_STAGE hotplug-d2-key-up",
        "hotplug-d2-release-timeout",
    )?;
    hid_qmp::send_named_keys(&mut qmp, &mut reader, &["d"], false, "d2-key-up")
}

fn command(
    qmp: &mut std::os::unix::net::UnixStream,
    reader: &mut super::qmp::Reader,
    command: &[u8],
    action: &'static str,
) -> Result<(), ConduitosError> {
    super::qmp::request(qmp, reader, command, action)
}

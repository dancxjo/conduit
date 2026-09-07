//! Bounded QEMU FTDI peer topology and its ordinary-product Line evidence.

use std::{
    os::unix::net::UnixStream,
    path::Path,
    process::Child,
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::{journey_records, ConduitosError};

pub(super) const QEMU_CONTROLLER: &str = "qemu-xhci,id=conduitos-xhci,p2=3,p3=0";
pub(super) const QEMU_DEVICE: &str =
    "usb-serial,id=conduitos-usb-line,chardev=conduitos-usb-line,bus=conduitos-xhci.0,port=3";
const EXPECTED_PEER_HOST: &str = "host/qemu-product-journey-ftdi-peer";
const EXPECTED_PEER_BOOT: &str = "boot/qemu-product-journey-ftdi-peer/1";

pub(super) struct ConnectedPeer {
    _stream: UnixStream,
}

pub(super) struct Evidence {
    pub line_id: String,
    pub binding_id: String,
    pub base_instance_id: String,
    pub state_sign_id: String,
    pub lost_state_sign_id: String,
    pub peer_host_id: String,
    pub peer_boot_id: String,
}

pub(super) fn chardev(socket: &Path) -> String {
    format!(
        "socket,id=conduitos-usb-line,path={},server=on,wait=on",
        socket.to_string_lossy()
    )
}

pub(super) fn connect(socket: &Path, child: &mut Child) -> Result<ConnectedPeer, ConduitosError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(ConnectedPeer { _stream: stream }),
            Err(_error) if Instant::now() < deadline => {
                if child
                    .try_wait()
                    .map_err(|status| {
                        ConduitosError::refusal(
                            "product-journey-qemu-wait-failed",
                            status.to_string(),
                        )
                    })?
                    .is_some()
                {
                    return Err(ConduitosError::refusal(
                        "product-journey-usb-line-peer-unavailable",
                        "QEMU exited before publishing its FTDI peer socket",
                    ));
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                return Err(ConduitosError::refusal(
                    "product-journey-usb-line-peer-unavailable",
                    error.to_string(),
                ));
            }
        }
    }
}

pub(super) fn evidence(serial: &str) -> Result<Evidence, ConduitosError> {
    let records = journey_records::usb_line(serial)?;
    if records.len() != 2 {
        return Err(ConduitosError::refusal(
            "product-journey-usb-line-record-count",
            "ordinary product must publish one current and one lost FTDI Line state",
        ));
    }
    let record = &records[0];
    let lost = &records[1];
    if record.get("status").and_then(Value::as_str) != Some("current")
        || text(record, "sink_host_id")? != EXPECTED_PEER_HOST
        || text(record, "sink_boot_id")? != EXPECTED_PEER_BOOT
        || lost.get("status").and_then(Value::as_str) != Some("lost")
        || lost.get("line_id") != record.get("line_id")
        || lost.get("binding_id") != record.get("binding_id")
        || lost.get("source_boot_id") != record.get("source_boot_id")
        || lost.get("stale_current_refused").and_then(Value::as_bool) != Some(true)
    {
        return Err(ConduitosError::refusal(
            "product-journey-usb-line-not-current",
            "FTDI Line current/lost lifecycle or connected peer identity is invalid",
        ));
    }
    Ok(Evidence {
        line_id: text(record, "line_id")?,
        binding_id: text(record, "binding_id")?,
        base_instance_id: text(record, "base_instance_id")?,
        state_sign_id: text(record, "state_sign_id")?,
        lost_state_sign_id: text(lost, "state_sign_id")?,
        peer_host_id: text(record, "sink_host_id")?,
        peer_boot_id: text(record, "sink_boot_id")?,
    })
}

fn text(record: &Value, field: &str) -> Result<String, ConduitosError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| ConduitosError::refusal("product-journey-usb-line-identity-missing", field))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_requires_one_current_exact_peer() {
        let serial = format!(concat!(
            "CONDUIT_USB_LINE_SIGN {{\"status\":\"current\",\"line_id\":\"line/1\",\"binding_id\":\"binding/1\",\"base_instance_id\":\"base/1\",\"state_sign_id\":\"sign/1\",\"source_boot_id\":\"boot/source\",\"sink_host_id\":\"{}\",\"sink_boot_id\":\"{}\"}}\n",
            "CONDUIT_USB_LINE_SIGN {{\"status\":\"lost\",\"line_id\":\"line/1\",\"binding_id\":\"binding/1\",\"source_boot_id\":\"boot/source\",\"state_sign_id\":\"sign/lost/1\",\"stale_current_refused\":true}}\n"
        ), EXPECTED_PEER_HOST, EXPECTED_PEER_BOOT);
        let record = evidence(&serial).unwrap();
        assert_eq!(record.line_id, "line/1");
        assert!(evidence(&serial.replace("current", "lost")).is_err());
    }
}

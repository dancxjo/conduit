//! Source-owned HID class exchange over the existing control proof boundary.
//! Attachment grants remain explicit; this helper offers no ordinary input.
use super::{
    control_proof_plan::ControlProofSubject,
    device_probe_proof_plan::{self, DeviceProbeProofRefusal},
};
use crate::protocol_source::PreparedProtocolArtifact;

pub fn prepare(
    subject: &ControlProofSubject<'_>,
) -> Result<PreparedProtocolArtifact, DeviceProbeProofRefusal> {
    let source = alloc::format!(
        "{}\n{}",
        include_str!("../../plots/usb/control-types.conduit"),
        include_str!("../../plots/usb/hid-boot-control.conduit"),
    );
    device_probe_proof_plan::prepare_source(subject, source, "usb-hid-select-boot")
}

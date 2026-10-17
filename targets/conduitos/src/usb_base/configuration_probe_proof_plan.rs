//! Source configuration exchange under the same explicit descriptor-proof root.
//! This fixture offers no discovered device/interface/endpoint authority.
use super::{
    control_proof_plan::ControlProofSubject,
    device_probe_proof_plan::{self, DeviceProbeProofRefusal},
};
use crate::protocol_source::PreparedProtocolArtifact;

pub const ADDITIONAL_LOCAL_SIGN_ITEMS: u16 = 60000;
pub const ADDITIONAL_REMOTE_SIGN_ITEMS: u16 = 1024;

pub fn prepare(
    subject: &ControlProofSubject<'_>,
) -> Result<PreparedProtocolArtifact, DeviceProbeProofRefusal> {
    device_probe_proof_plan::prepare_source(
        subject,
        alloc::format!(
            "{}\n{}\n{}",
            include_str!("../../plots/usb/control-types.conduit"),
            include_str!("../../plots/usb/configuration-walk.conduit"),
            include_str!("../../plots/usb/configuration-probe.conduit")
        ),
        "usb-configuration-probe",
    )
}

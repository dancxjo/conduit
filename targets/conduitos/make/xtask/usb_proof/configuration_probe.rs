//! Configuration receipt validation uses its exact Source and native sign budget.
use super::{
    device_probe::{self, DescriptorProbe, DeviceProbeSign},
    ConduitosError,
};
use conduitos::usb_base::control_proof_plan::ControlProofSubject;

pub(super) fn extract(
    serial: &str,
    subject: &ControlProofSubject<'_>,
    initial: (usize, u32),
) -> Result<DeviceProbeSign, ConduitosError> {
    device_probe::extract_for(serial, subject, initial, DescriptorProbe::Configuration)
}

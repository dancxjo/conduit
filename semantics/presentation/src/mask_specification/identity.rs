use alloc::{format, string::String};
use conduit_core::PortDirection;
use sha2::{Digest, Sha256};

use super::{
    MaskBoundaryPort, MaskCordSpecification, MaskSpecificationError, MaskSpecificationId,
    MaskStageSpecification, MAX_MASK_IDENTITY_BYTES,
};

pub(super) fn validate_identity(value: &str) -> Result<(), MaskSpecificationError> {
    if value.is_empty()
        || value.len() > MAX_MASK_IDENTITY_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/'))
    {
        Err(MaskSpecificationError::InvalidIdentity)
    } else {
        Ok(())
    }
}

pub(super) fn bind_specification(
    name: &str,
    revision: u64,
    stages: &[MaskStageSpecification],
    cords: &[MaskCordSpecification],
    boundaries: &[MaskBoundaryPort],
) -> MaskSpecificationId {
    let mut canonical = format!("conduit.presentation/mask-specification@1\n{name}\n{revision}\n");
    for stage in stages {
        canonical.push_str(stage.stage_id.as_str());
        canonical.push('\n');
        canonical.push_str(stage.kind_id.as_str());
        canonical.push('\n');
        canonical.push_str(stage.kind_contract_revision.as_str());
        canonical.push('\n');
        for port in stage.inputs.iter().chain(&stage.outputs) {
            canonical.push_str(port.port_id.as_str());
            canonical.push(':');
            canonical.push_str(port.value_kind.as_str());
            canonical.push(':');
            canonical.push_str(match port.direction {
                PortDirection::Input => "input",
                PortDirection::Output => "output",
            });
            canonical.push(':');
            canonical.push_str(port.temporal.as_str());
            canonical.push('\n');
        }
    }
    for cord in cords {
        canonical.push_str(cord.source_stage_id.as_str());
        canonical.push(':');
        canonical.push_str(cord.source_port_id.as_str());
        canonical.push_str(">>");
        canonical.push_str(cord.sink_stage_id.as_str());
        canonical.push(':');
        canonical.push_str(cord.sink_port_id.as_str());
        canonical.push(':');
        canonical.push_str(cord.value_kind.as_str());
        canonical.push('\n');
    }
    for boundary in boundaries {
        canonical.push_str(&format!(
            "{:?}:{}:{}\n",
            boundary.role,
            boundary.stage_id.as_str(),
            boundary.port_id.as_str()
        ));
    }
    let digest = Sha256::digest(canonical.as_bytes());
    let mut identity = String::from("mask/");
    for byte in digest {
        identity.push_str(&format!("{byte:02x}"));
    }
    MaskSpecificationId(identity)
}

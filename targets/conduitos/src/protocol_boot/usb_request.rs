//! Bounded local Root input, separate from portable Source and from possession.
use super::request::{MAXIMUM_REQUEST_BYTES, ProtocolBootInput, ProtocolBootRequestRefusal};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ROOT_MODULE_COMMAND: &[u8] = b"conduit.protocol/usb-root-request@1";
pub const REQUEST_SCHEMA: &str = "conduit.conduitos/usb-protocol-root-request@1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbProtocolBootRequest {
    pub schema: String,
    pub entry: String,
    pub package_sha256: String,
    /// Administrative approval is meaningful only through trusted local boot
    /// input. Parsing this flag is not review, firmware handoff or possession.
    pub administrator_approval: bool,
    pub firmware_handoff: bool,
    pub electrical_attachment_approved: bool,
    pub root_port: u8,
    pub attachment_epoch: u32,
    pub maximum_control_operations: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_read: Option<UsbEndpointBootRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_write: Option<UsbEndpointBootRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub isochronous_out: Option<UsbIsochronousEndpointBootRequest>,
    /// Root reserves retained remote lifecycle history before Play. This is
    /// storage admission, not additional device authority or operation fuel.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub additional_remote_sign_items: u16,
    pub maximum_steps: u32,
    pub inputs: Vec<ProtocolBootInput>,
}

/// An explicit administrator-selected transport configuration, not class policy.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbEndpointBootRequest {
    pub address: u8,
    pub transfer_type: u8,
    pub maximum_packet: u16,
    pub interval: u8,
    pub endpoint_epoch: u32,
    pub maximum_operations: u32,
}

/// Explicit USB2 full-speed 1ms ISO transport admission, independent of class policy.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbIsochronousEndpointBootRequest {
    pub address: u8,
    pub maximum_packet: u16,
    pub interval: u8,
    pub endpoint_epoch: u32,
    pub maximum_operations: u32,
}
impl UsbIsochronousEndpointBootRequest {
    fn valid(&self) -> bool {
        self.address & 0xf0 == 0
            && self.address != 0
            && (1..=256).contains(&self.maximum_packet)
            && self.interval == 1
            && self.endpoint_epoch != 0
            && (1..=4096).contains(&self.maximum_operations)
    }
}

fn is_zero(value: &u16) -> bool {
    *value == 0
}

impl UsbEndpointBootRequest {
    fn valid(&self, inbound: bool) -> bool {
        (self.address & 0x80 != 0) == inbound
            && self.address & 0x70 == 0
            && self.address & 0x0f != 0
            && matches!(self.transfer_type, 2 | 3)
            && (1..=1024).contains(&self.maximum_packet)
            && (self.transfer_type != 3 || self.interval != 0)
            && self.endpoint_epoch != 0
            && (1..=4096).contains(&self.maximum_operations)
    }
}

impl UsbProtocolBootRequest {
    /// Decode inert administrative input. The native Root independently checks
    /// actual hardware configuration and claims exclusive resources before
    /// admitting providers or issuing any capability.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolBootRequestRefusal> {
        use ProtocolBootRequestRefusal as Error;
        if bytes.is_empty() || bytes.len() > MAXIMUM_REQUEST_BYTES {
            return Err(Error::Bounds);
        }
        let request: Self = serde_json::from_slice(bytes).map_err(|_| Error::Encoding)?;
        if request.schema != REQUEST_SCHEMA {
            return Err(Error::Schema);
        }
        if !request.administrator_approval
            || !request.firmware_handoff
            || !request.electrical_attachment_approved
        {
            return Err(Error::Approval);
        }
        if request.entry.is_empty()
            || request.entry.len() > 128
            || request.package_sha256.len() != 64
            || !request
                .package_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || request.root_port == 0
            || request.attachment_epoch == 0
            || request.maximum_control_operations == 0
            || request.maximum_control_operations > 4096
            || request.maximum_steps == 0
            || request.maximum_steps > 1000000
            || request
                .endpoint_read
                .as_ref()
                .is_some_and(|endpoint| !endpoint.valid(true))
            || request
                .endpoint_write
                .as_ref()
                .is_some_and(|endpoint| !endpoint.valid(false) || request.endpoint_read.is_none())
            || request.isochronous_out.as_ref().is_some_and(|endpoint| {
                !endpoint.valid()
                    || request.endpoint_read.is_some()
                    || request.endpoint_write.is_some()
            })
            || request.inputs.len() > 16
            || request.inputs.iter().any(|input| {
                input.port.is_empty()
                    || input.port.len() > 128
                    || input.canonical_bytes.is_empty()
                    || input.canonical_bytes.len()
                        > crate::protocol_source::MAXIMUM_PROTOCOL_FORE_BYTES as usize
            })
            || request.inputs.iter().enumerate().any(|(index, input)| {
                request.inputs[..index]
                    .iter()
                    .any(|previous| previous.port == input.port)
            })
        {
            return Err(Error::Bounds);
        }
        Ok(request)
    }

    pub fn bind_package(&self, bytes: &[u8]) -> Result<(), ProtocolBootRequestRefusal> {
        let actual = alloc::format!("{:x}", Sha256::digest(bytes));
        if !self.package_sha256.eq_ignore_ascii_case(&actual) {
            return Err(ProtocolBootRequestRefusal::SourceBinding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

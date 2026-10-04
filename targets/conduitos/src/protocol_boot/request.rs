//! Bounded local Root input, separate from portable Source and from possession.
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ROOT_MODULE_COMMAND: &[u8] = b"conduit.protocol/root-request@1";
pub const MAXIMUM_REQUEST_BYTES: usize = 64 * 1024;
pub const REQUEST_SCHEMA: &str = "conduit.conduitos/protocol-root-request@1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolBootInput {
    pub port: String,
    /// Canonical typed bytes; the exact kind is taken from the checked Fore.
    pub canonical_bytes: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolBootRequest {
    pub schema: String,
    pub entry: String,
    pub package_sha256: String,
    /// Administrative approval is meaningful only through trusted local boot
    /// input. Parsing this flag is not review, firmware handoff or possession.
    pub administrator_approval: bool,
    pub firmware_handoff: bool,
    pub electrical_attachment_approved: bool,
    pub pci_bus: u8,
    pub pci_device: u8,
    pub pci_function: u8,
    pub minimum_address: u8,
    pub maximum_address: u8,
    pub maximum_polls: u32,
    pub maximum_bus_operations: u32,
    pub maximum_clock_operations: u32,
    pub clock_lifetime_millis: u32,
    pub maximum_steps: u32,
    pub inputs: Vec<ProtocolBootInput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolBootRequestRefusal {
    Bounds,
    Encoding,
    Schema,
    Approval,
    SourceBinding,
}

impl ProtocolBootRequest {
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
            || request.pci_device >= 32
            || request.pci_function >= 8
            || request.minimum_address < 8
            || request.maximum_address > 119
            || request.minimum_address > request.maximum_address
            || request.maximum_polls == 0
            || request.maximum_polls > 65536
            || request.maximum_bus_operations == 0
            || request.maximum_bus_operations > 4096
            || request.maximum_clock_operations == 0
            || request.maximum_clock_operations > 4096
            || request.clock_lifetime_millis == 0
            || request.clock_lifetime_millis > 60000
            || request.maximum_steps == 0
            || request.maximum_steps > 1000000
            || request.inputs.len() > 16
            || request.inputs.iter().any(|input| {
                input.port.is_empty()
                    || input.port.len() > 128
                    || input.canonical_bytes.is_empty()
                    || input.canonical_bytes.len() > 4096
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

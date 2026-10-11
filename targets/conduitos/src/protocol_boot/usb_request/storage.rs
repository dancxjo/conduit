//! Independently approved read-only block use in trusted local Root input.
use super::*;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbStorageRequest {
    pub administrator_approval: bool,
    pub maximum_operations: u32,
}
impl UsbStorageRequest {
    pub(super) fn validate(&self) -> Result<(), ProtocolBootRequestRefusal> {
        if !self.administrator_approval {
            return Err(ProtocolBootRequestRefusal::Approval);
        }
        if !(1..=16).contains(&self.maximum_operations) {
            return Err(ProtocolBootRequestRefusal::Bounds);
        }
        Ok(())
    }
}

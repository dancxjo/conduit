//! Independently approved read-only block use in trusted local Root input.
use super::*;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbStorageRequest {
    pub administrator_approval: bool,
    pub maximum_operations: u32,
    /// Separately approve one fresh read-only authorization after recovery.
    #[serde(default, skip_serializing_if = "is_false")]
    pub read_only_reentry_approval: bool,
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

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_storage_input_does_not_implicitly_approve_reentry() {
        let input = r#"{"administrator_approval":true,"maximum_operations":1}"#;
        let request: UsbStorageRequest = serde_json::from_str(input).unwrap();
        assert!(!request.read_only_reentry_approval);
        assert_eq!(serde_json::to_string(&request).unwrap(), input);
        request.validate().unwrap();
    }
    #[test]
    fn explicit_reentry_does_not_replace_original_storage_approval() {
        let request: UsbStorageRequest = serde_json::from_str(
            r#"{"administrator_approval":false,"maximum_operations":1,"read_only_reentry_approval":true}"#
        ).unwrap();
        assert!(request.validate().is_err());
    }
}

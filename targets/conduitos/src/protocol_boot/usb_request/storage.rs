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
    /// Trusted Root selects exact expanded Gear IDs; these names grant nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_bindings: Option<StoragePlacementBindings>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoragePlacementBindings {
    pub authorize: String,
    pub settle: String,
    pub reentry_authorize: String,
    pub reentry_settle: String,
}
impl StoragePlacementBindings {
    fn valid(&self) -> bool {
        let names = [
            &self.authorize,
            &self.settle,
            &self.reentry_authorize,
            &self.reentry_settle,
        ];
        names.iter().enumerate().all(|(index, name)| {
            !name.is_empty()
                && name.len() <= 256
                && !name.chars().any(char::is_control)
                && !names[..index].contains(name)
        })
    }
}
impl UsbStorageRequest {
    pub(in crate::protocol_boot) fn validate(&self) -> Result<(), ProtocolBootRequestRefusal> {
        if !self.administrator_approval {
            return Err(ProtocolBootRequestRefusal::Approval);
        }
        if !(1..=16).contains(&self.maximum_operations) {
            return Err(ProtocolBootRequestRefusal::Bounds);
        }
        if self.read_only_reentry_approval != self.placement_bindings.is_some()
            || self
                .placement_bindings
                .as_ref()
                .is_some_and(|bindings| !bindings.valid())
        {
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
    fn approved(names: [&str; 4], approval: bool) -> UsbStorageRequest {
        serde_json::from_value(serde_json::json!({"administrator_approval": true, "maximum_operations": 1,
            "read_only_reentry_approval": approval, "placement_bindings": {
                "authorize": names[0], "settle": names[1], "reentry_authorize": names[2], "reentry_settle": names[3]
            }})).unwrap()
    }
    #[test]
    fn reentry_requires_four_unique_bounded_explicit_names() {
        approved(["a", "s", "new-a", "new-s"], true)
            .validate()
            .unwrap();
        assert!(
            approved(["a", "s", "new-a", "new-s"], false)
                .validate()
                .is_err()
        );
        for names in [
            ["a", "s", "a", "new-s"],
            ["", "s", "new-a", "new-s"],
            ["a\n", "s", "new-a", "new-s"],
        ] {
            assert!(approved(names, true).validate().is_err());
        }
        assert!(
            approved([&"a".repeat(257), "s", "new-a", "new-s"], true)
                .validate()
                .is_err()
        );
        let missing: UsbStorageRequest = serde_json::from_str(r#"{"administrator_approval":true,"maximum_operations":1,"read_only_reentry_approval":true}"#).unwrap();
        assert!(missing.validate().is_err());
        assert!(serde_json::from_str::<UsbStorageRequest>(r#"{"administrator_approval":true,"maximum_operations":1,"read_only_reentry_approval":true,"placement_bindings":{"authorize":"a","settle":"s","reentry_authorize":"new-a"}}"#).is_err());
    }
}

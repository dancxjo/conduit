//! Inert, separately approved network scope in trusted local Root input.
use super::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbEthernetNetworkRequest {
    pub administrator_approval: bool,
    pub mac: [u8; 6],
    pub guest_address: [u8; 4],
    pub prefix_length: u8,
    pub gateway: [u8; 4],
    pub remote_address: [u8; 4],
    pub remote_port: u16,
    pub local_port: u16,
    pub maximum_polls: u32,
    pub maximum_duration_millis: u32,
    pub maximum_operations: u32,
    pub request: Vec<u8>,
    pub maximum_response_bytes: u16,
    pub expected_response_bytes: u16,
}

impl UsbEthernetNetworkRequest {
    pub(super) fn validate(&self) -> Result<(), ProtocolBootRequestRefusal> {
        if !self.administrator_approval {
            return Err(ProtocolBootRequestRefusal::Approval);
        }
        if self.mac == [0; 6]
            || self.mac[0] & 1 != 0
            || self.guest_address == [0; 4]
            || self.gateway == [0; 4]
            || self.remote_address == [0; 4]
            || !(1..=32).contains(&self.prefix_length)
            || self.remote_port == 0
            || self.local_port == 0
            || !(1..=1_000_000).contains(&self.maximum_polls)
            || !(1..=60_000).contains(&self.maximum_duration_millis)
            || !(1..=4096).contains(&self.maximum_operations)
            || self.request.is_empty()
            || self.request.len() > 2048
            || !(1..=2048).contains(&self.maximum_response_bytes)
            || self.expected_response_bytes == 0
            || self.expected_response_bytes > self.maximum_response_bytes
        {
            return Err(ProtocolBootRequestRefusal::Bounds);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> UsbEthernetNetworkRequest {
        UsbEthernetNetworkRequest {
            administrator_approval: true,
            mac: [2, 0, 0, 0, 0, 1],
            guest_address: [10, 0, 2, 15],
            prefix_length: 24,
            gateway: [10, 0, 2, 2],
            remote_address: [10, 0, 2, 2],
            remote_port: 8000,
            local_port: 49152,
            maximum_polls: 4096,
            maximum_duration_millis: 30_000,
            maximum_operations: 64,
            request: alloc::vec![1],
            maximum_response_bytes: 1024,
            expected_response_bytes: 1,
        }
    }
    #[test]
    fn network_approval_is_independent_and_cannot_be_omitted_or_false() {
        let mut request = scope();
        assert_eq!(request.validate(), Ok(()));
        request.administrator_approval = false;
        assert_eq!(
            request.validate(),
            Err(ProtocolBootRequestRefusal::Approval)
        );
        let mut encoded = serde_json::to_value(scope()).unwrap();
        encoded
            .as_object_mut()
            .unwrap()
            .remove("administrator_approval");
        assert!(serde_json::from_value::<UsbEthernetNetworkRequest>(encoded).is_err());
    }
    #[test]
    fn network_scope_and_storage_are_finite_before_possession() {
        for change in [
            "mac",
            "guest_address",
            "gateway",
            "remote_address",
            "prefix_length",
            "remote_port",
            "local_port",
            "maximum_polls",
            "maximum_duration_millis",
            "maximum_operations",
            "request",
            "maximum_response_bytes",
            "expected_response_bytes",
        ] {
            let mut encoded = serde_json::to_value(scope()).unwrap();
            encoded[change] = match change {
                "mac" => serde_json::json!([1, 0, 0, 0, 0, 1]),
                "guest_address" | "gateway" | "remote_address" => serde_json::json!([0, 0, 0, 0]),
                "request" => serde_json::json!([]),
                _ => serde_json::json!(0),
            };
            let request: UsbEthernetNetworkRequest = serde_json::from_value(encoded).unwrap();
            assert_eq!(
                request.validate(),
                Err(ProtocolBootRequestRefusal::Bounds),
                "{change}"
            );
        }
        let mut request = scope();
        request.request = alloc::vec![1; 2049];
        assert_eq!(request.validate(), Err(ProtocolBootRequestRefusal::Bounds));
        request = scope();
        request.maximum_polls = 1_000_001;
        assert_eq!(request.validate(), Err(ProtocolBootRequestRefusal::Bounds));
        request = scope();
        request.maximum_duration_millis = 60_001;
        assert_eq!(request.validate(), Err(ProtocolBootRequestRefusal::Bounds));
        request = scope();
        request.maximum_operations = 4097;
        assert_eq!(request.validate(), Err(ProtocolBootRequestRefusal::Bounds));
        request = scope();
        request.expected_response_bytes = 1025;
        assert_eq!(request.validate(), Err(ProtocolBootRequestRefusal::Bounds));
    }
}

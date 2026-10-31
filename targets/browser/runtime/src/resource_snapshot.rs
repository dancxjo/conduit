//! Browser public path for the shared immutable Resource record codec.
pub use conduit_resource_snapshot::*;
#[cfg(test)]
#[path = "../../../../mechanisms/io/resource-snapshot/src/tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
mod wire_tests {
    #[test]
    fn shared_operation_keeps_the_browser_storage_request_shape() {
        let request = super::SnapshotStorageRequest {
            effect_kind: "resource-read",
            key: "resource/example",
            record: None,
        };
        assert_eq!(
            serde_json::to_value(request).unwrap(),
            serde_json::json!({"effect_kind":"resource-read", "key":"resource/example", "record":null})
        );
    }
}

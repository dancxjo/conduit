//! Pure checked intent for an explicitly selected Linux storage directory.
//! This declaration opens nothing and issues no operation or resource authority.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};
pub const BODY_STORAGE_IMPLEMENTATION: &str = "linux/body-storage-directory@1";
pub const BODY_STORAGE_BASE_KIND: &str = "storage/protected-file";
pub fn body_storage_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, max| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::DataLabel { maximum: max },
    };
    let integer = |key: &str, min, max| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(min),
        rule: KindConfigurationRule::U64Range {
            minimum: min,
            maximum: max,
        },
    };
    BaseInitializationContract {
        fields: vec![
            text("root_directory", 4096),
            text("pool_namespace", 96),
            integer("maximum_snapshot_records", 1, 64),
            integer("maximum_record_bytes", 1, 4096),
            integer("generation_slots", 1, 64),
            integer("publication_slots", 1, 16),
            integer("reader_leases", 1, 16),
            integer("state_slots", 1, 1),
            integer("maximum_state_bytes", 1, 4096),
            integer("total_disk_bytes", 1, 64 * 1024 * 1024),
            integer("preparation_bytes", 1, 64 * 1024 * 1024),
        ],
    }
}
pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: BODY_STORAGE_BASE_KIND.into(),
        implementation_id: BODY_STORAGE_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![
            "std/x86_64/computer".into(),
            "std/armv6/raspberry-pi-model-b-plus-v1.2".into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
            "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
            "std/aarch64/raspberry-pi-zero-2-w-rev-1.0".into(),
            "std/aarch64/raspberry-pi-zero-2-wh-rev-1.0".into(),
            "std/aarch64/raspberry-pi-4-model-b-rev-1.5-4gb".into(),
        ],
        prerequisites: Vec::new(),
        build_feature: None,
    }
}

//! Explicit pools for a projection-only initialized Base; no acquisition authority.
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationOffer, KindConfigurationField,
    KindConfigurationRule,
};
pub(super) fn contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: ["face_pool", "memory_pool"]
            .into_iter()
            .map(|key| KindConfigurationField {
                key: key.into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 128 },
            })
            .collect(),
    }
}
pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: conduit_std_offers::REPORTED_GENERATION_FACE_BASE.into(),
        implementation_id: conduit_std_offers::REPORTED_GENERATION_FACE_DRIVER.into(),
        implementation_revision: 1,
        target_patterns: vec![
            crate::HOSTED_TARGET_ID.into(),
            "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
        ],
        prerequisites: vec![],
        build_feature: None,
    }
}

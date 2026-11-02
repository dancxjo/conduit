//! Checked construction intent only: no encounter, readiness or authority is created.
use conduit_body_make::deployment_runtime_catalog::{
    creche_std_offer, CRECHE_IMPLEMENTATION, CRECHE_RESOURCE,
};
use conduit_host_make::{
    BaseInitializationContract, ConfigurationValue, ImplementationMetadata, ImplementationOffer,
    KindConfigurationField, KindConfigurationRule, PrerequisiteNode,
};

pub const CRECHE_ENCOUNTER_BASE_KIND: &str = "creche/encounter";

/// Every field is required. The slot count is not byte admission; actual owner
/// preparation and workspace custody must precede any runtime advertisement.
pub fn creche_encounter_initialization_contract() -> BaseInitializationContract {
    let count = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "slot_identity".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 256 },
            },
            KindConfigurationField {
                key: "image_manifest".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 1024 },
            },
            KindConfigurationField {
                key: "image_manifest_sha256".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 64 },
            },
            KindConfigurationField {
                key: "artifact".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 1024 },
            },
            KindConfigurationField {
                key: "artifact_sha256".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 64 },
            },
            KindConfigurationField {
                key: "host_configuration".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 1024 },
            },
            KindConfigurationField {
                key: "host_configuration_sha256".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 64 },
            },
            count("maximum_artifact_bytes", 1, 4 * 1024 * 1024 * 1024),
            count("maximum_manifest_bytes", 1, 1024 * 1024),
            count("maximum_evidence_bytes", 1, 64 * 1024 * 1024),
            count("maximum_polls", 1, 1_000_000),
            count("minimum_poll_interval_ms", 1, 1000),
        ],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: CRECHE_ENCOUNTER_BASE_KIND.into(),
        implementation_id: CRECHE_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![crate::HOSTED_TARGET_ID.into()],
        prerequisites: Vec::new(),
        build_feature: None,
    }
}

pub(super) fn metadata() -> ImplementationMetadata {
    let offer = creche_std_offer();
    ImplementationMetadata {
        kind: offer.kind_id.as_str().into(),
        contract_revision: offer.kind_contract_revision.as_str().into(),
        targets: vec![crate::HOSTED_TARGET_ID.into()],
        prerequisites: offer
            .host_calls
            .iter()
            .map(|call| PrerequisiteNode::HostCall(call.contract_id.as_str().into()))
            .chain(
                offer
                    .resource_requirements
                    .iter()
                    .map(|resource| PrerequisiteNode::Resource(resource.class_id.as_str().into())),
            )
            .collect(),
    }
}

pub(super) fn resource_dependency() -> (PrerequisiteNode, Vec<PrerequisiteNode>) {
    (
        PrerequisiteNode::Resource(CRECHE_RESOURCE.into()),
        vec![PrerequisiteNode::Base(CRECHE_ENCOUNTER_BASE_KIND.into())],
    )
}

#[cfg(test)]
mod tests;

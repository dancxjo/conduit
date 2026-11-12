//! Checked single-generation file intent; no directory, grant or Ready provider.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    ImplementationMetadata, ImplementationOffer, KindConfigurationField, KindConfigurationRule,
    PrerequisiteNode,
};
use conduit_std_offers::*;
fn targets() -> Vec<String> {
    vec![
        crate::HOSTED_TARGET_ID.into(),
        "std/aarch64/raspberry-pi-3-model-b-plus-rev-1.3".into(),
        "std/aarch64/raspberry-pi-5-model-b-rev-1.1-8gb".into(),
    ]
}
pub fn text_file_initialization_contract() -> BaseInitializationContract {
    let count = |key: &str, default, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(default),
        rule: KindConfigurationRule::U64Range {
            minimum: 1,
            maximum,
        },
    };
    BaseInitializationContract {
        fields: vec![
            count(
                "maximum_text_bytes",
                u64::from(DURABLE_TEXT_MAXIMUM_BYTES),
                u64::from(DURABLE_TEXT_MAXIMUM_BYTES),
            ),
            count("preparation_bytes", 1, u64::from(u32::MAX)),
            count("retained_bytes", 1, u64::from(u32::MAX)),
        ],
    }
}
pub(super) fn offer(write: bool) -> ImplementationOffer {
    ImplementationOffer {
        base_kind: DURABLE_TEXT_BASE_KIND.into(),
        implementation_id: implementation(write).into(),
        implementation_revision: 1,
        target_patterns: targets(),
        prerequisites: vec![],
        build_feature: None,
    }
}
pub(super) fn metadata(write: bool) -> ImplementationMetadata {
    let contract = durable_text_contract(write);
    ImplementationMetadata {
        kind: contract.kind_id.as_str().into(),
        contract_revision: contract.kind_contract_revision.as_str().into(),
        targets: targets(),
        prerequisites: vec![
            PrerequisiteNode::HostCall(
                if write {
                    DURABLE_TEXT_PUBLISH_CALL
                } else {
                    DURABLE_TEXT_READ_CALL
                }
                .into(),
            ),
            PrerequisiteNode::Resource(DURABLE_TEXT_RESOURCE.into()),
            PrerequisiteNode::Driver(implementation(write).into()),
        ],
    }
}
pub(super) fn resource_dependency() -> (PrerequisiteNode, Vec<PrerequisiteNode>) {
    (
        PrerequisiteNode::Resource(DURABLE_TEXT_RESOURCE.into()),
        vec![PrerequisiteNode::Base(DURABLE_TEXT_BASE_KIND.into())],
    )
}
fn implementation(write: bool) -> &'static str {
    if write {
        DURABLE_TEXT_SAVE_IMPLEMENTATION
    } else {
        DURABLE_TEXT_LOAD_IMPLEMENTATION
    }
}
/// Exact Source-bound finite construction parameters. Directory custody, the
/// immutable reference and the existing snapshot authority grant remain separate
/// Root inputs; none can be inferred from this description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedTextFileInitialization {
    pub configuration_id: String,
    pub driver: String,
    pub write: bool,
    pub maximum_text_bytes: u32,
    pub preparation_bytes: u64,
    pub retained_bytes: u64,
}
impl CheckedTextFileInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        if checked.configuration().target.os.as_deref() != Some("linux")
            || checked.profile().target.family != "std"
        {
            return Err("durable Text initialization requires a selected Linux/std Host".into());
        }
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|b| b.kind == DURABLE_TEXT_BASE_KIND);
        let base = bases.next().ok_or("no selected durable Text Base")?;
        if bases.next().is_some() || !base.implementations.is_empty() {
            return Err("durable Text requires one explicitly pinned Base".into());
        }
        let mut selected = checked
            .resolved_bases()
            .iter()
            .filter(|(kind, _)| kind == DURABLE_TEXT_BASE_KIND);
        let (_, driver) = selected
            .next()
            .ok_or("durable Text driver is not resolved")?;
        let write = match driver.as_str() {
            DURABLE_TEXT_SAVE_IMPLEMENTATION => true,
            DURABLE_TEXT_LOAD_IMPLEMENTATION => false,
            _ => return Err("unsupported durable Text driver".into()),
        };
        if selected.next().is_some() || base.implementation.as_deref() != Some(driver.as_str()) {
            return Err("durable Text requires one exact selected driver".into());
        }
        let mut capabilities = checked.profile().capabilities.iter().filter(|c| {
            [DURABLE_TEXT_SAVE_KIND, DURABLE_TEXT_LOAD_KIND].contains(&c.kind.as_str())
        });
        let capability = capabilities
            .next()
            .ok_or("durable Text operation is not explicitly selected")?;
        let kind = if write {
            DURABLE_TEXT_SAVE_KIND
        } else {
            DURABLE_TEXT_LOAD_KIND
        };
        if capabilities.next().is_some()
            || capability.implementation != *driver
            || capability.kind != kind
            || capability.contract_revision != DURABLE_TEXT_CONTRACT_REVISION
        {
            return Err("durable Text capability differs from its selected driver".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("missing durable Text initialization")?
            .checked_values(&text_file_initialization_contract())?;
        let count = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(value)) => Ok(*value),
            _ => Err("durable Text count has wrong type".to_string()),
        };
        let result = Self {
            configuration_id: checked.configuration_id().into(),
            driver: driver.clone(),
            write,
            maximum_text_bytes: u32::try_from(count("maximum_text_bytes")?)
                .map_err(|_| "durable Text extent overflow")?,
            preparation_bytes: count("preparation_bytes")?,
            retained_bytes: count("retained_bytes")?,
        };
        if result.retained_bytes > result.preparation_bytes
            || result.preparation_bytes > checked.configuration().limits.heap_arena_bytes
        {
            return Err(
                "durable Text preparation omits retention or exceeds declared memory".into(),
            );
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests;

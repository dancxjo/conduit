//! Checked compiler equipment intent. No Source grant, process or Ready offer.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    ImplementationMetadata, ImplementationOffer, KindConfigurationField, KindConfigurationRule,
    PrerequisiteNode,
};
use conduit_std_offers::{
    SOURCE_CHECK_BASE_KIND, SOURCE_CHECK_CALL, SOURCE_CHECK_IMPLEMENTATION, SOURCE_CHECK_KIND,
    SOURCE_CHECK_RESOURCE,
};
use std::path::PathBuf;

pub fn source_check_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(String::new()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    let count = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("executable", 4096),
            text("executable_sha256", 64),
            count("maximum_executable_bytes", 1, 512 * 1024 * 1024),
            count("address_space_bytes", 8 * 1024 * 1024, 1024 * 1024 * 1024),
            count("cpu_seconds", 1, 30),
            count("deadline_milliseconds", 1, 30000),
            count("maximum_startup_bytes", 1, 6 * 1024 * 1024 + 16384),
            count("preparation_bytes", 1, u32::MAX as u64),
            count("retained_bytes", 1, u32::MAX as u64),
            count("request_bytes", 1, 262144),
            count("reply_bytes", 1, 65536),
        ],
    }
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: SOURCE_CHECK_BASE_KIND.into(),
        implementation_id: SOURCE_CHECK_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![crate::HOSTED_TARGET_ID.into()],
        prerequisites: vec![],
        build_feature: None,
    }
}
pub(super) fn metadata() -> ImplementationMetadata {
    ImplementationMetadata {
        kind: SOURCE_CHECK_KIND.into(),
        contract_revision: SOURCE_CHECK_CALL.into(),
        targets: vec![crate::HOSTED_TARGET_ID.into()],
        prerequisites: vec![
            PrerequisiteNode::HostCall(SOURCE_CHECK_CALL.into()),
            PrerequisiteNode::Resource(SOURCE_CHECK_RESOURCE.into()),
        ],
    }
}
pub(super) fn resource_dependency() -> (PrerequisiteNode, Vec<PrerequisiteNode>) {
    (
        PrerequisiteNode::Resource(SOURCE_CHECK_RESOURCE.into()),
        vec![PrerequisiteNode::Base(SOURCE_CHECK_BASE_KIND.into())],
    )
}

/// Finite construction selection only; the artifact must still be verified and
/// actual preparation admitted before installing a provider. Source is supplied
/// separately under Body authority, never embedded in this equipment request.
#[derive(Debug, Clone)]
pub struct CheckedSourceCheckInitialization {
    pub configuration_id: String,
    pub executable: PathBuf,
    pub executable_sha256: [u8; 32],
    pub maximum_executable_bytes: u64,
    pub address_space_bytes: u64,
    pub cpu_seconds: u64,
    pub deadline_milliseconds: u64,
    pub maximum_startup_bytes: u64,
    pub preparation_bytes: u64,
    pub retained_bytes: u64,
    pub request_bytes: u64,
    pub reply_bytes: u64,
}
impl CheckedSourceCheckInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        if checked.configuration().target.os.as_deref() != Some("linux") {
            return Err("Source compiler requires Linux".into());
        }
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|b| b.kind == SOURCE_CHECK_BASE_KIND);
        let base = bases.next().ok_or("no selected Source compiler Base")?;
        if bases.next().is_some()
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, implementation)| {
                    kind == SOURCE_CHECK_BASE_KIND && implementation == SOURCE_CHECK_IMPLEMENTATION
                })
        {
            return Err("Source compiler requires one exact selected implementation".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("missing compiler initialization")?
            .checked_values(&source_check_initialization_contract())?;
        let text = |key: &str| match values.get(key) {
            Some(ConfigurationValue::Text(value)) => Ok(value.as_str()),
            _ => Err("compiler text field has wrong type".to_string()),
        };
        let count = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(value)) => Ok(*value),
            _ => Err("compiler count field has wrong type".to_string()),
        };
        let executable = PathBuf::from(text("executable")?);
        if !executable.is_absolute() || text("executable")?.contains('\0') {
            return Err("compiler executable requires an absolute path".into());
        }
        let digest = text("executable_sha256")?;
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("compiler digest requires 32 hexadecimal bytes".into());
        }
        let mut executable_sha256 = [0; 32];
        for (index, byte) in executable_sha256.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&digest[index * 2..index * 2 + 2], 16)
                .map_err(|_| "invalid compiler digest")?;
        }
        let result = Self {
            configuration_id: checked.configuration_id().into(),
            executable,
            executable_sha256,
            maximum_executable_bytes: count("maximum_executable_bytes")?,
            address_space_bytes: count("address_space_bytes")?,
            cpu_seconds: count("cpu_seconds")?,
            deadline_milliseconds: count("deadline_milliseconds")?,
            maximum_startup_bytes: count("maximum_startup_bytes")?,
            preparation_bytes: count("preparation_bytes")?,
            retained_bytes: count("retained_bytes")?,
            request_bytes: count("request_bytes")?,
            reply_bytes: count("reply_bytes")?,
        };
        // Necessary lower bounds, not a measurement of full Root workspace.
        let minimum =
            result.address_space_bytes + result.maximum_startup_bytes + result.retained_bytes;
        if result.preparation_bytes < minimum
            || result.preparation_bytes > checked.configuration().limits.heap_arena_bytes
        {
            return Err("compiler preparation exceeds declared memory or omits child/startup/retained backing".into());
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests;

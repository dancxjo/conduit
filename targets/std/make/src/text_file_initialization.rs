//! Explicit finite immutable Text residence intent; no filesystem effects or authority.
use conduit_core::{
    BoundedResourceRef, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    ImplementationMetadata, ImplementationOffer, KindConfigurationField, KindConfigurationRule,
    PrerequisiteNode,
};
use std::path::PathBuf;
pub const TEXT_FILE_BASE_KIND: &str = "storage/immutable-text";
pub fn text_file_initialization_contract() -> BaseInitializationContract {
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
            text("directory", 4096),
            text("identity", 64),
            text("version", 64),
            text("content_profile", 64),
            text("access", 32),
            count("content_bytes", 1, 4096),
            count("disk_capacity_bytes", 8192, 1024 * 1024),
        ],
    }
}
pub(super) fn offers() -> Vec<ImplementationOffer> {
    [
        conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION,
        conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION,
    ]
    .into_iter()
    .map(|implementation| ImplementationOffer {
        base_kind: TEXT_FILE_BASE_KIND.into(),
        implementation_id: implementation.into(),
        implementation_revision: 1,
        target_patterns: vec!["std/*/*".into()],
        prerequisites: vec![],
        build_feature: None,
    })
    .collect()
}
pub(super) fn metadata(write: bool) -> ImplementationMetadata {
    ImplementationMetadata {
        kind: if write {
            "data/save/text"
        } else {
            "data/load/text"
        }
        .into(),
        contract_revision: "conduit.data/text-generation@2".into(),
        targets: vec!["std/*/*".into()],
        prerequisites: vec![
            PrerequisiteNode::HostCall(
                if write {
                    conduit_std_offers::DURABLE_TEXT_PUBLISH_CALL
                } else {
                    conduit_std_offers::DURABLE_TEXT_READ_CALL
                }
                .into(),
            ),
            PrerequisiteNode::Resource("data/immutable-generation@1".into()),
        ],
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedTextFileInitialization {
    pub configuration_id: String,
    pub directory: PathBuf,
    pub reference: BoundedResourceRef,
    pub write: bool,
    pub disk_capacity_bytes: u64,
}
impl CheckedTextFileInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        if checked.configuration().target.os.as_deref() != Some("linux") {
            return Err("immutable Text residence requires Linux".into());
        }
        let mut bases = checked
            .configuration()
            .bases
            .iter()
            .filter(|base| base.kind == TEXT_FILE_BASE_KIND);
        let base = bases.next().ok_or("no selected immutable Text residence")?;
        if bases.next().is_some() {
            return Err("immutable Text residence requires one selected generation".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("immutable Text residence requires explicit initialization")?
            .checked_values(&text_file_initialization_contract())?;
        let text = |key: &str| match values.get(key) {
            Some(ConfigurationValue::Text(value)) => Ok(value.as_str()),
            _ => Err("immutable Text initialization text field"),
        };
        let number = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(value)) => Ok(*value),
            _ => Err("immutable Text initialization count field"),
        };
        let directory = PathBuf::from(text("directory")?);
        if !directory.is_absolute()
            || directory
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("immutable Text requires an absolute selected directory".into());
        }
        let write = match text("access")? {
            "write-candidate-publish" => true,
            "read-published" => false,
            _ => return Err("immutable Text access must be explicit".into()),
        };
        let implementation = if write {
            conduit_std_offers::DURABLE_TEXT_SAVE_IMPLEMENTATION
        } else {
            conduit_std_offers::DURABLE_TEXT_LOAD_IMPLEMENTATION
        };
        if !checked
            .resolved_bases()
            .iter()
            .any(|(kind, selected)| kind == TEXT_FILE_BASE_KIND && selected == implementation)
        {
            return Err("immutable Text access differs from selected implementation".into());
        }
        if text("content_profile")? != "value/text" {
            return Err("immutable Text requires value/text content".into());
        }
        let reference = BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest(text("identity")?)?),
            content_profile: "value/text".into(),
            access_class: "data/immutable-generation@1".into(),
            extent: ResourceExtent {
                bytes: number("content_bytes")?,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(digest(text("version")?)?),
                expires_at: None,
            },
        };
        reference
            .validate()
            .map_err(|e| format!("immutable Text reference: {e:?}"))?;
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            directory,
            reference,
            write,
            disk_capacity_bytes: number("disk_capacity_bytes")?,
        })
    }
}
fn digest(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("immutable Text identity requires 64 lowercase hex digits".into());
    }
    let mut result = [0; 32];
    for (index, value) in result.iter_mut().enumerate() {
        *value =
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(result)
}
